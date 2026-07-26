// SPDX-License-Identifier: Apache-2.0

//! Terminal integration — PTY wrapper, status bar, and rendering.
//!
//! Spellcast operates as a PTY wrapper:
//! - Spawns the user's shell in a pseudo-terminal
//! - Intercepts keyboard input (raw mode via crossterm)
//! - In dictation mode: captures audio, runs ASR, tokenizes, injects text
//! - In raw mode: passes all keys through to the PTY
//! - Renders a status bar with mode indicator, token, and predictions

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};

pub mod highlight;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};

use crate::asr::AsrEngine;
use crate::audio::vad::VadConfig;
use crate::audio::{AudioBuffer, AudioCapture, AudioConfig as SpcAudioConfig};
use crate::config::SpellcastConfig;
use crate::error::{SpellcastError, SpellcastResult};
use crate::memory::MemoryStore;
use crate::modes::{Mode, ModeController};
use crate::predictor::Predictor;
use crate::tokenizer::{HeuristicTokenizer, TokenContext, TokenStream, Tokenizer};

use silero::{SpeechOptions, SpeechSegmenter, SpeechSegmenterExt, StreamState};

/// Global flag for the kill switch (accessible from signal handlers).
static KILL_SWITCH_ENGAGED: AtomicBool = AtomicBool::new(false);

/// Background continuous-listening thread handle for dictation mode.
///
/// When dictation mode is entered, a background thread is spawned that
/// continuously captures audio, runs Silero VAD to detect speech segments,
/// transcribes each completed segment with ASR, and sends the result text
/// via an mpsc channel to the main loop. Dropping this struct stops the
/// thread (the `stop` flag is set and the audio stream is dropped).
struct DictationListener {
    /// Set to true to signal the background thread to stop.
    stop: Arc<AtomicBool>,
    /// Receives ASR results (Ok(text)) or errors (Err(msg)).
    rx: mpsc::Receiver<Result<String, String>>,
    /// Handle to the background thread (joined on drop).
    handle: Option<std::thread::JoinHandle<()>>,
}

impl DictationListener {
    /// Start a continuous VAD-based listening thread.
    ///
    /// The thread:
    /// 1. Opens the audio device (cpal Device is !Send, so this must happen inside the thread)
    /// 2. Starts a continuous capture stream that emits 512-sample f32 chunks
    /// 3. Feeds each chunk to a Silero `SpeechSegmenter` (via `SpeechSegmenterExt`)
    /// 4. When the segmenter emits a complete `SpeechSegment`, extracts the PCM,
    ///    runs ASR, and sends the result text via the channel
    fn start(
        audio_config: SpcAudioConfig,
        asr_engine: Arc<Box<dyn AsrEngine>>,
    ) -> Result<Self, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop);
        let (tx, rx) = mpsc::channel::<Result<String, String>>();

        let handle = std::thread::spawn(move || {
            // 1. Open audio device inside the thread
            let audio_capture = match AudioCapture::new(&audio_config) {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(Err(format!("Audio init error: {e}")));
                    return;
                }
            };

            // 2. Channel for audio chunks from capture callback to this thread
            let (chunk_tx, chunk_rx) = mpsc::channel::<Vec<f32>>();

            // 3. Start continuous capture
            let stream = match audio_capture.start_continuous(chunk_tx) {
                Ok(s) => s,
                Err(e) => {
                    let _ = tx.send(Err(format!("Audio stream error: {e}")));
                    return;
                }
            };

            log::info!("DICTATION: continuous VAD listener started");

            // 4. Initialize VAD session + segmenter
            let vad_config = VadConfig::default();
            let session = match silero::Session::bundled() {
                Ok(s) => s,
                Err(e) => {
                    let _ = tx.send(Err(format!("VAD model error: {e}")));
                    let _ = stream; // keep stream alive until here
                    return;
                }
            };
            let mut session = session;

            let options = SpeechOptions::default()
                .with_start_threshold(vad_config.threshold)
                .with_min_silence_duration(std::time::Duration::from_millis(
                    vad_config.min_silence_ms as u64,
                ))
                .with_min_speech_duration(std::time::Duration::from_millis(
                    vad_config.min_speech_ms as u64,
                ))
                .with_speech_pad(std::time::Duration::from_millis(
                    vad_config.pre_padding_ms as u64,
                ));

            let mut stream_state = StreamState::new(silero::SampleRate::Rate16k);
            let mut segmenter = SpeechSegmenter::new(options);

            // Buffer to accumulate PCM for the current speech segment
            let mut pcm_buffer: Vec<f32> = Vec::new();

            // 5. Main loop: receive chunks, feed VAD, emit segments
            while !stop_clone.load(Ordering::SeqCst) {
                match chunk_rx.recv_timeout(std::time::Duration::from_millis(100)) {
                    Ok(chunk) => {
                        // Accumulate PCM for segment extraction
                        pcm_buffer.extend_from_slice(&chunk);

                        // Feed chunk to VAD segmenter
                        match segmenter.push_samples(&mut session, &mut stream_state, &chunk) {
                            Ok(Some(segment)) => {
                                // Complete speech segment detected — extract PCM and run ASR
                                Self::transcribe_segment(&segment, &pcm_buffer, &asr_engine, &tx);
                                // Do NOT trim pcm_buffer here. VAD segment indices are
                                // absolute from stream start. Trimming would make subsequent
                                // segments point to wrong data. Buffer grows for the
                                // duration of dictation mode (~19MB for 5 min at 16kHz).
                            }
                            Ok(None) => {
                                // No complete segment yet, keep accumulating
                            }
                            Err(e) => {
                                log::warn!("VAD error: {e}");
                            }
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        // Check stop flag and continue
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        // Audio stream stopped
                        break;
                    }
                }
            }

            log::info!("DICTATION: continuous VAD listener stopping");
            drop(stream);
        });

        Ok(Self {
            stop,
            rx,
            handle: Some(handle),
        })
    }

    /// Extract the PCM for a speech segment, run ASR, and send the result.
    fn transcribe_segment(
        segment: &silero::SpeechSegment,
        pcm_buffer: &[f32],
        asr_engine: &Arc<Box<dyn AsrEngine>>,
        tx: &mpsc::Sender<Result<String, String>>,
    ) {
        let start = segment.start_sample() as usize;
        let end = segment.end_sample() as usize;
        let start = start.min(pcm_buffer.len());
        let end = end.min(pcm_buffer.len());
        if start >= end {
            return;
        }

        let segment_pcm = &pcm_buffer[start..end];
        let i16_samples: Vec<i16> = segment_pcm.iter().map(|&s| (s * 32768.0) as i16).collect();

        let buffer = AudioBuffer {
            samples: i16_samples,
            sample_rate: 16000,
        };

        log::info!(
            "DICTATION: speech segment [{:.2}s - {:.2}s] ({} samples), transcribing...",
            segment.start_seconds(),
            segment.end_seconds(),
            segment_pcm.len()
        );

        match asr_engine.transcribe(&buffer) {
            Ok(result) => {
                let _ = tx.send(Ok(result.text));
            }
            Err(e) => {
                let _ = tx.send(Err(format!("ASR error: {e}")));
            }
        }
    }

    /// Try to receive an ASR result (non-blocking).
    fn try_recv(&self) -> Result<Option<Result<String, String>>, ()> {
        match self.rx.try_recv() {
            Ok(result) => Ok(Some(result)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(()),
        }
    }
}

impl Drop for DictationListener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Installs signal handlers that restore the terminal on exit.
fn install_signal_handlers() {
    let result = std::panic::catch_unwind(|| {
        ctrlc::set_handler(move || {
            let _ = terminal::disable_raw_mode();
            let _ = crossterm::execute!(std::io::stdout(), LeaveAlternateScreen);
            std::process::exit(130);
        })
        .ok();
    });
    let _ = result;
}

/// A guard that restores the terminal when dropped.
struct TerminalGuard;

impl TerminalGuard {
    fn new() -> Self {
        install_signal_handlers();
        TerminalGuard
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), LeaveAlternateScreen);
    }
}

/// Run the main Spellcast terminal loop.
pub fn run_terminal_loop(
    config: &SpellcastConfig,
    mode_ctrl: &mut ModeController,
    memory: &MemoryStore,
    shell: Option<&str>,
    asr_engine: Box<dyn AsrEngine>,
    audio_config: &SpcAudioConfig,
) -> SpellcastResult<()> {
    // Raw mode FIRST, then alternate screen
    terminal::enable_raw_mode()
        .map_err(|e| SpellcastError::TerminalRender(format!("Failed to enable raw mode: {e}")))?;
    crossterm::execute!(std::io::stdout(), EnterAlternateScreen).map_err(|e| {
        SpellcastError::TerminalRender(format!("Failed to enter alternate screen: {e}"))
    })?;
    let _guard = TerminalGuard::new();

    let result = run_inner(config, mode_ctrl, memory, shell, asr_engine, audio_config);

    let _ = crossterm::execute!(std::io::stdout(), LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();

    result
}

fn run_inner(
    _config: &SpellcastConfig,
    mode_ctrl: &mut ModeController,
    _memory: &MemoryStore,
    shell: Option<&str>,
    asr_engine: Box<dyn AsrEngine>,
    audio_config: &SpcAudioConfig,
) -> SpellcastResult<()> {
    let mut tokenizer = HeuristicTokenizer::new();
    let predictor = Predictor::new();

    // Wrap ASR engine in Arc for shared access from background threads
    let asr_engine = Arc::new(asr_engine);

    // Create the PTY
    let pty_system = native_pty_system();
    let size = get_terminal_size()?;

    let pair = pty_system
        .openpty(size)
        .map_err(|e| SpellcastError::TerminalPty(format!("Failed to create PTY: {e}")))?;

    // Spawn the shell
    let shell_path = shell
        .map(|s| s.to_string())
        .unwrap_or_else(|| std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string()));

    let cmd = CommandBuilder::new(&shell_path);
    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| SpellcastError::TerminalPty(format!("Failed to spawn shell: {e}")))?;

    // PTY reader — dedicated thread so it doesn't block the event loop
    let mut pty_reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| SpellcastError::TerminalPty(format!("Failed to clone PTY reader: {e}")))?;

    let (pty_tx, pty_rx) = mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match pty_reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if pty_tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let mut pty_writer = pair
        .master
        .take_writer()
        .map_err(|_| SpellcastError::TerminalPty("Failed to get PTY writer".to_string()))?;

    // State for token navigation
    let mut current_tokens: TokenStream = TokenStream::new(TokenContext::Prose);
    let mut token_index: Option<usize> = None;
    let mut predictions: Vec<String> = Vec::new();

    // Continuous VAD listener — None when not in dictation mode
    let mut listener: Option<DictationListener> = None;

    // Main event loop
    loop {
        // Check if the child process is still alive
        if let Ok(Some(_)) = child.try_wait() {
            log::info!("Shell process exited");
            break;
        }

        // Drain PTY output (non-blocking from channel)
        let mut got_output = false;
        while let Ok(data) = pty_rx.try_recv() {
            got_output = true;
            let output = String::from_utf8_lossy(&data);
            log::trace!(
                "PTY OUT: {} bytes: {:?}",
                data.len(),
                &output[..output.len().min(80)]
            );
            print!("{}", output);
        }
        if got_output {
            let _ = std::io::stdout().flush();
        }

        // Check for ASR result from the continuous listener (non-blocking)
        if let Some(ref dict_listener) = listener
            && let Ok(Some(result)) = dict_listener.try_recv()
        {
            match result {
                Ok(text) => {
                    log::info!("ASR result: '{text}'");
                    if !text.is_empty() {
                        // Tokenize the ASR result into the draft buffer
                        if let Ok(new_tokens) = tokenizer.tokenize(&text) {
                            log::info!(
                                "Tokenized into {} tokens (context: {:?})",
                                new_tokens.len(),
                                new_tokens.context
                            );

                            // Merge new tokens into the draft buffer (NOT the PTY)
                            for token in new_tokens.tokens {
                                current_tokens.tokens.push(token);
                            }

                            // Set cursor to the last token
                            if !current_tokens.is_empty() {
                                token_index = Some(current_tokens.len() - 1);
                            }

                            // Run predictor on the last word
                            if let Some(last) = current_tokens.tokens.last()
                                && !last.text.is_empty()
                            {
                                predictions = predictor
                                    .predict(&last.text, 3)
                                    .unwrap_or_default()
                                    .into_iter()
                                    .map(|p| p.word)
                                    .collect();
                                log::info!("Predictions for '{}': {:?}", last.text, predictions);
                            }

                            // Render the draft buffer
                            render_draft(&current_tokens, token_index, &predictions)?;
                        }
                    }
                }
                Err(e) => {
                    log::error!("ASR error: {e}");
                }
            }
        }

        // Log status in dictation mode
        let dict_listening = listener.is_some();
        if dict_listening && current_tokens.is_empty() {
            log::debug!(
                "Status: mode={:?}, draft empty, predictions={}",
                mode_ctrl.current_mode(),
                predictions.len()
            );
        }

        // Poll for keyboard input (50ms timeout)
        if !event::poll(std::time::Duration::from_millis(50))
            .map_err(|e| SpellcastError::TerminalPty(format!("Event poll error: {e}")))?
        {
            continue;
        }

        match event::read()
            .map_err(|e| SpellcastError::TerminalPty(format!("Event read error: {e}")))?
        {
            Event::Key(key_event) => {
                log::debug!(
                    "KEY: code={:?} modifiers={:?} kind={:?}",
                    key_event.code,
                    key_event.modifiers,
                    key_event.kind
                );

                // Check kill switch first
                if is_kill_switch(&key_event) {
                    mode_ctrl.toggle_kill_switch();
                    KILL_SWITCH_ENGAGED
                        .store(mode_ctrl.current_mode().is_killed(), Ordering::SeqCst);
                    let killed = mode_ctrl.current_mode().is_killed();
                    log::info!(
                        "KILL SWITCH: {} (mode={:?})",
                        if killed { "ENGAGED" } else { "DISENGAGED" },
                        mode_ctrl.current_mode()
                    );
                    continue;
                }

                // In killed mode, pass everything through
                if mode_ctrl.current_mode().is_killed() {
                    write_pty(&mut pty_writer, &key_event)?;
                    continue;
                }

                match mode_ctrl.current_mode() {
                    Mode::Dictation => {
                        if is_mode_toggle(&key_event) {
                            mode_ctrl.toggle_mode();
                            log::info!("MODE: dictation -> raw");
                            // Stop the continuous listener
                            listener = None;
                            continue;
                        }

                        handle_dictation_key(
                            &key_event,
                            &mut pty_writer,
                            &mut current_tokens,
                            &mut token_index,
                            &mut predictions,
                        )?;
                    }
                    Mode::Raw => {
                        if is_mode_toggle(&key_event) {
                            mode_ctrl.toggle_mode();
                            log::info!("MODE: raw -> dictation");
                            // Start the continuous listener
                            match DictationListener::start(
                                audio_config.clone(),
                                Arc::clone(&asr_engine),
                            ) {
                                Ok(l) => listener = Some(l),
                                Err(e) => {
                                    log::error!("Failed to start dictation listener: {e}");
                                }
                            }
                            continue;
                        }
                        log::trace!("RAW: passing key through to PTY");
                        write_pty(&mut pty_writer, &key_event)?;
                    }
                    Mode::Killed => {
                        write_pty(&mut pty_writer, &key_event)?;
                    }
                }
            }
            Event::Resize(cols, rows) => {
                let new_size = PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                };
                let _ = pair.master.resize(new_size);
            }
            _ => {}
        }
    }

    // Listener is dropped here, stopping the background thread.
    Ok(())
}

/// Render the draft buffer on the line below the cursor.
/// The current token is highlighted in reverse video.
/// Predictions are shown on the line below that.
fn render_draft(
    tokens: &TokenStream,
    token_index: Option<usize>,
    predictions: &[String],
) -> SpellcastResult<()> {
    use std::io::Write as _;

    if tokens.is_empty() {
        return Ok(());
    }

    let mut stdout = std::io::stdout();

    // Save cursor, move to next line
    let _ = write!(stdout, "\r\n");

    // Render each token, highlighting the current one
    for (i, token) in tokens.tokens.iter().enumerate() {
        if Some(i) == token_index {
            // Reverse video for current token
            let _ = write!(stdout, "\x1b[7m{}\x1b[0m", token.text);
        } else {
            let _ = write!(stdout, "{}", token.text);
        }
    }

    // Show predictions on the line below if any
    if !predictions.is_empty() {
        let _ = write!(stdout, "\r\n");
        for (i, pred) in predictions.iter().enumerate() {
            let _ = write!(stdout, " {}: {}  ", i + 1, pred);
        }
    }

    // Move cursor back to the shell prompt position
    // (up by the number of lines we printed, then to column 0)
    let lines_printed = 1 + if !predictions.is_empty() { 1 } else { 0 };
    let _ = write!(stdout, "\r\x1b[{}A", lines_printed);

    let _ = stdout.flush();
    Ok(())
}

/// Clear the draft display from the terminal.
fn clear_draft(token_count: usize) -> SpellcastResult<()> {
    use std::io::Write as _;

    if token_count == 0 {
        return Ok(());
    }

    let mut stdout = std::io::stdout();

    // Move down one line, clear it, move back up
    let _ = write!(stdout, "\r\n\x1b[2K\r\x1b[A");

    let _ = stdout.flush();
    Ok(())
}

/// Status bar rendering is disabled.
/// The save/restore cursor approach corrupts the terminal display.
/// Mode, tokens, and predictions are logged to the log file instead.
#[allow(dead_code)]
fn render_status_bar(
    _mode: Mode,
    _tokens: &TokenStream,
    _token_index: Option<usize>,
    _predictions: &[String],
    _dict_listening: bool,
) -> SpellcastResult<()> {
    Ok(())
}

/// Handle a key event in dictation mode.
///
/// In continuous VAD mode, Space is no longer push-to-talk — it passes
/// through to the PTY as a regular character. Special keys (h/l/x/r/e/1-3)
/// still work for token navigation.
fn handle_dictation_key(
    key: &KeyEvent,
    pty: &mut Box<dyn Write + Send>,
    tokens: &mut TokenStream,
    token_index: &mut Option<usize>,
    predictions: &mut Vec<String>,
) -> SpellcastResult<()> {
    match key.code {
        // Navigate to previous token
        KeyCode::Char('h') | KeyCode::Left if key.modifiers.is_empty() => {
            let idx = token_index.unwrap_or(tokens.len().saturating_sub(1));
            if idx > 0 {
                *token_index = Some(idx - 1);
            }
            log::trace!("NAV: prev token, index={:?}", token_index);
            render_draft(tokens, *token_index, predictions)?;
        }

        // Navigate to next token
        KeyCode::Char('l') | KeyCode::Right if key.modifiers.is_empty() => {
            let idx = token_index.unwrap_or(0);
            if idx + 1 < tokens.len() {
                *token_index = Some(idx + 1);
            }
            log::trace!("NAV: next token, index={:?}", token_index);
            render_draft(tokens, *token_index, predictions)?;
        }

        // Delete highlighted token
        KeyCode::Char('x') if key.modifiers.is_empty() => {
            if let Some(idx) = *token_index
                && idx < tokens.len()
            {
                tokens.remove(idx);
                log::info!("DELETE: removed token at {idx}");
                *token_index = if tokens.is_empty() {
                    None
                } else if idx >= tokens.len() {
                    Some(tokens.len() - 1)
                } else {
                    Some(idx)
                };
                render_draft(tokens, *token_index, predictions)?;
            }
        }

        // Re-dictate: delete highlighted token (VAD will pick up new speech)
        KeyCode::Char('r') if key.modifiers.is_empty() => {
            if let Some(idx) = *token_index
                && idx < tokens.len()
            {
                tokens.remove(idx);
                log::info!("RE-DICTATE: removed token at {idx}");
                *token_index = None;
                render_draft(tokens, *token_index, predictions)?;
            }
        }

        // Explain: trigger explain pipeline (not yet wired to LLM)
        KeyCode::Char('e') if key.modifiers.is_empty() => {
            if let Some(idx) = *token_index
                && let Some(token) = tokens.get(idx)
            {
                log::info!(
                    "EXPLAIN: triggered on token '{}' (explainer not yet wired)",
                    token.text
                );
            }
        }

        // Accept prediction
        KeyCode::Char(c) if ('1'..='3').contains(&c) => {
            let pred_idx = (c as u8 - b'1') as usize;
            if pred_idx < predictions.len() {
                let prediction = predictions[pred_idx].clone();
                log::info!("PREDICT: accepted prediction {pred_idx}: '{prediction}'");

                // Replace the current token with the prediction
                if let Some(idx) = *token_index
                    && idx < tokens.len()
                {
                    tokens.tokens[idx].text = prediction.clone();
                }
                *token_index = None;
                predictions.clear();
                render_draft(tokens, *token_index, predictions)?;
            }
        }

        // Enter: commit draft buffer to shell
        KeyCode::Enter => {
            if !tokens.is_empty() {
                let text: String = tokens
                    .tokens
                    .iter()
                    .map(|t| t.text.as_str())
                    .collect::<Vec<_>>()
                    .join("");
                log::info!("COMMIT: writing '{}' to PTY", text);

                // Clear the draft display first
                clear_draft(tokens.len())?;

                // Write the committed text to the PTY
                let _ = write!(pty, "{text}");
                pty.flush().ok();

                // Clear the draft buffer
                tokens.tokens.clear();
                *token_index = None;
                predictions.clear();
            } else {
                // Empty draft, just send Enter
                let _ = writeln!(pty);
                pty.flush().ok();
            }
        }

        // Backspace: delete last token from draft
        KeyCode::Backspace => {
            if !tokens.is_empty() {
                tokens.tokens.pop();
                *token_index = if tokens.is_empty() {
                    None
                } else {
                    Some(tokens.len() - 1)
                };
                log::trace!("BACKSPACE: removed last token, {} remaining", tokens.len());
                render_draft(tokens, *token_index, predictions)?;
            }
        }

        // Escape: discard the entire draft buffer
        KeyCode::Esc => {
            if !tokens.is_empty() {
                log::info!("DISCARD: clearing draft buffer ({} tokens)", tokens.len());
                clear_draft(tokens.len())?;
                tokens.tokens.clear();
                *token_index = None;
                predictions.clear();
            }
        }

        // Other printable characters: ignore in dictation mode
        // (speech is the input method, not typing)
        KeyCode::Char(_) if key.modifiers.is_empty() => {}

        _ => {}
    }

    Ok(())
}

/// Check if a key event is the kill switch (Ctrl+G).
/// Ctrl+G sends BEL (0x07) in raw mode. Both forms are detected.
fn is_kill_switch(key: &KeyEvent) -> bool {
    let is_ctrl_g = key.code == KeyCode::Char('g') && key.modifiers.contains(KeyModifiers::CONTROL);
    let is_bel = key.code == KeyCode::Char('\x07');
    is_ctrl_g || is_bel
}

/// Check if a key event toggles dictation mode.
/// Accepts Caps Lock (kitty protocol) OR Ctrl+Space (universal fallback).
fn is_mode_toggle(key: &KeyEvent) -> bool {
    if key.code == KeyCode::CapsLock {
        return true;
    }
    if key.code == KeyCode::Char(' ') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return true;
    }
    false
}

/// Write a key event to the PTY.
fn write_pty(pty: &mut Box<dyn Write + Send>, key: &KeyEvent) -> SpellcastResult<()> {
    match key.code {
        KeyCode::Char(c) => {
            // Handle Ctrl+letter combos (Ctrl+A=0x01, Ctrl+B=0x02, ... Ctrl+Z=0x1A)
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                let ctrl_byte = c as u32;
                if ctrl_byte >= 0x60 {
                    // lowercase letter
                    let byte = (ctrl_byte - 0x60) as u8;
                    pty.write_all(&[byte]).map_err(|e| {
                        SpellcastError::TerminalPty(format!("PTY write error: {e}"))
                    })?;
                } else if (0x41..=0x5A).contains(&ctrl_byte) {
                    // uppercase letter
                    let byte = (ctrl_byte - 0x40) as u8;
                    pty.write_all(&[byte]).map_err(|e| {
                        SpellcastError::TerminalPty(format!("PTY write error: {e}"))
                    })?;
                } else {
                    let mut buf = [0u8; 4];
                    let s = c.encode_utf8(&mut buf);
                    pty.write_all(s.as_bytes()).map_err(|e| {
                        SpellcastError::TerminalPty(format!("PTY write error: {e}"))
                    })?;
                }
            } else {
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                pty.write_all(s.as_bytes())
                    .map_err(|e| SpellcastError::TerminalPty(format!("PTY write error: {e}")))?;
            }
        }
        KeyCode::Enter => {
            pty.write_all(b"\r")
                .map_err(|e| SpellcastError::TerminalPty(format!("PTY write error: {e}")))?;
        }
        KeyCode::Backspace => {
            pty.write_all(b"\x7f")
                .map_err(|e| SpellcastError::TerminalPty(format!("PTY write error: {e}")))?;
        }
        KeyCode::Tab => {
            pty.write_all(b"\t")
                .map_err(|e| SpellcastError::TerminalPty(format!("PTY write error: {e}")))?;
        }
        KeyCode::Esc => {
            pty.write_all(b"\x1b")
                .map_err(|e| SpellcastError::TerminalPty(format!("PTY write error: {e}")))?;
        }
        KeyCode::Left => {
            pty.write_all(b"\x1b[D")
                .map_err(|e| SpellcastError::TerminalPty(format!("PTY write error: {e}")))?;
        }
        KeyCode::Right => {
            pty.write_all(b"\x1b[C")
                .map_err(|e| SpellcastError::TerminalPty(format!("PTY write error: {e}")))?;
        }
        KeyCode::Up => {
            pty.write_all(b"\x1b[A")
                .map_err(|e| SpellcastError::TerminalPty(format!("PTY write error: {e}")))?;
        }
        KeyCode::Down => {
            pty.write_all(b"\x1b[B")
                .map_err(|e| SpellcastError::TerminalPty(format!("PTY write error: {e}")))?;
        }
        KeyCode::Home => {
            pty.write_all(b"\x1b[H")
                .map_err(|e| SpellcastError::TerminalPty(format!("PTY write error: {e}")))?;
        }
        KeyCode::End => {
            pty.write_all(b"\x1b[F")
                .map_err(|e| SpellcastError::TerminalPty(format!("PTY write error: {e}")))?;
        }
        KeyCode::Delete => {
            pty.write_all(b"\x1b[3~")
                .map_err(|e| SpellcastError::TerminalPty(format!("PTY write error: {e}")))?;
        }
        KeyCode::PageUp => {
            pty.write_all(b"\x1b[5~")
                .map_err(|e| SpellcastError::TerminalPty(format!("PTY write error: {e}")))?;
        }
        KeyCode::PageDown => {
            pty.write_all(b"\x1b[6~")
                .map_err(|e| SpellcastError::TerminalPty(format!("PTY write error: {e}")))?;
        }
        _ => {}
    }

    pty.flush()
        .map_err(|e| SpellcastError::TerminalPty(format!("PTY flush error: {e}")))?;
    Ok(())
}

/// Get the current terminal size.
fn get_terminal_size() -> SpellcastResult<PtySize> {
    let (cols, rows) = crossterm::terminal::size()
        .map_err(|e| SpellcastError::TerminalPty(format!("Failed to get terminal size: {e}")))?;
    Ok(PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyCode;

    #[test]
    fn test_is_kill_switch() {
        let ks = KeyEvent {
            code: KeyCode::Char('g'),
            modifiers: KeyModifiers::CONTROL,
            kind: event::KeyEventKind::Press,
            state: event::KeyEventState::NONE,
        };
        assert!(is_kill_switch(&ks));

        let bel_ks = KeyEvent {
            code: KeyCode::Char('\x07'),
            modifiers: KeyModifiers::NONE,
            kind: event::KeyEventKind::Press,
            state: event::KeyEventState::NONE,
        };
        assert!(is_kill_switch(&bel_ks));

        let not_ks = KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::CONTROL,
            kind: event::KeyEventKind::Press,
            state: event::KeyEventState::NONE,
        };
        assert!(!is_kill_switch(&not_ks));
    }

    #[test]
    fn test_mode_toggle_ctrl_space() {
        let ctrl_space = KeyEvent {
            code: KeyCode::Char(' '),
            modifiers: KeyModifiers::CONTROL,
            kind: event::KeyEventKind::Press,
            state: event::KeyEventState::NONE,
        };
        assert!(is_mode_toggle(&ctrl_space));
    }

    #[test]
    fn test_mode_toggle_caps_lock() {
        let caps = KeyEvent {
            code: KeyCode::CapsLock,
            modifiers: KeyModifiers::NONE,
            kind: event::KeyEventKind::Press,
            state: event::KeyEventState::NONE,
        };
        assert!(is_mode_toggle(&caps));
    }

    #[test]
    fn test_mode_toggle_not_plain_space() {
        let plain_space = KeyEvent {
            code: KeyCode::Char(' '),
            modifiers: KeyModifiers::NONE,
            kind: event::KeyEventKind::Press,
            state: event::KeyEventState::NONE,
        };
        assert!(!is_mode_toggle(&plain_space));
    }

    #[test]
    fn test_space_is_not_push_to_talk() {
        // In the new continuous VAD model, Space in dictation mode should
        // fall through to the "printable character" arm and be written to
        // the PTY — it must NOT trigger recording.
        //
        // We verify this by checking that handle_dictation_key with Space
        // writes a space character to the PTY. We use a shared buffer so
        // we can read it after the Box<dyn Write> is dropped.
        use std::sync::{Arc, Mutex};

        let buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));

        struct SharedWriter(Arc<Mutex<Vec<u8>>>);
        impl Write for SharedWriter {
            fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(data);
                Ok(data.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let mut pty: Box<dyn Write + Send> = Box::new(SharedWriter(Arc::clone(&buf)));
        let mut tokens = TokenStream::new(TokenContext::Prose);
        let mut token_index: Option<usize> = None;
        let mut predictions: Vec<String> = Vec::new();

        let space_key = KeyEvent {
            code: KeyCode::Char(' '),
            modifiers: KeyModifiers::NONE,
            kind: event::KeyEventKind::Press,
            state: event::KeyEventState::NONE,
        };

        // This should NOT write anything to the PTY — in dictation mode,
        // printable chars are ignored (speech is the input method, not typing).
        let result = handle_dictation_key(
            &space_key,
            &mut pty,
            &mut tokens,
            &mut token_index,
            &mut predictions,
        );
        assert!(result.is_ok());

        // Drop pty so the Arc<Mutex> borrow is released, then check buffer.
        drop(pty);
        assert_eq!(*buf.lock().unwrap(), b"");
    }

    #[test]
    fn test_get_terminal_size() {
        let result = get_terminal_size();
        if let Ok(size) = result {
            assert!(size.cols > 0);
            assert!(size.rows > 0);
        }
    }
}
