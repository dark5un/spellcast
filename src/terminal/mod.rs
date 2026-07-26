// SPDX-License-Identifier: Apache-2.0

//! Terminal integration — PTY wrapper, status bar, and rendering.
//!
//! Spellcast operates as a PTY wrapper:
//! - Spawns the user's shell in a pseudo-terminal
//! - Intercepts keyboard input (raw mode via crossterm)
//! - Injects processed text into the PTY
//! - Renders a status bar with mode indicator, token, and predictions

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};

pub mod highlight;

use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyModifiers, KeyboardEnhancementFlags,
    PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};

use crate::config::SpellcastConfig;
use crate::error::{SpellcastError, SpellcastResult};
use crate::memory::MemoryStore;
use crate::modes::{Mode, ModeController};
use crate::tokenizer::{HeuristicTokenizer, TokenContext, TokenStream};

/// Global flag for the kill switch (accessible from signal handlers).
static KILL_SWITCH_ENGAGED: AtomicBool = AtomicBool::new(false);

/// Installs signal handlers that restore the terminal on exit.
fn install_signal_handlers() {
    // Restore terminal on SIGTERM/SIGINT using ctrlc
    // We use a simple approach: set a flag that the main loop checks
    // This is a best-effort guard; the main code also restores on Drop.
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
        let _ = crossterm::execute!(std::io::stdout(), PopKeyboardEnhancementFlags);
        let _ = terminal::disable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), LeaveAlternateScreen);
    }
}

/// Run the main Spellcast terminal loop.
///
/// This function:
/// 1. Spawns the user's shell in a PTY
/// 2. Enters raw mode and starts the event loop
/// 3. Handles mode switching, token navigation, dictation, and explain
pub fn run_terminal_loop(
    config: &SpellcastConfig,
    mode_ctrl: &mut ModeController,
    _memory: &MemoryStore,
    shell: Option<&str>,
) -> SpellcastResult<()> {
    // Initialize terminal
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen).map_err(|e| {
        SpellcastError::TerminalRender(format!("Failed to enter alternate screen: {e}"))
    })?;
    crossterm::execute!(
        stdout,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    )
    .ok();
    terminal::enable_raw_mode()
        .map_err(|e| SpellcastError::TerminalRender(format!("Failed to enable raw mode: {e}")))?;
    let _guard = TerminalGuard::new();

    let result = run_inner(config, mode_ctrl, _memory, shell);

    // Explicit restore (guard catches panic paths)
    let _ = crossterm::execute!(std::io::stdout(), LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();

    result
}

fn run_inner(
    config: &SpellcastConfig,
    mode_ctrl: &mut ModeController,
    _memory: &MemoryStore,
    shell: Option<&str>,
) -> SpellcastResult<()> {
    // Create the tokenizer
    let tokenizer = HeuristicTokenizer::new();

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

    // Get PTY reader and writer
    let mut pty_reader = pair
        .master
        .try_clone_reader()
        .unwrap_or_else(|_| panic!("Failed to clone PTY reader"));

    let mut pty_writer = pair
        .master
        .take_writer()
        .map_err(|_| SpellcastError::TerminalPty("Failed to get PTY writer".to_string()))?;

    // State for token navigation
    let mut current_tokens: TokenStream = TokenStream::new(TokenContext::Prose);
    let mut token_index: Option<usize> = None;
    let mut pending_text: String = String::new();
    let mut predictions: Vec<String> = Vec::new();
    let _dictation_mode_active = false;

    // Main event loop
    loop {
        // Render status bar
        render_status_bar(
            mode_ctrl.current_mode(),
            &current_tokens,
            token_index,
            &predictions,
        )?;

        // Check if the child process is still alive
        if let Ok(Some(_exit_status)) = child.try_wait() {
            log::info!("Shell process exited");
            // Flush any remaining output from the PTY
            let mut buf = [0u8; 4096];
            while pty_reader.read(&mut buf).unwrap_or(0) > 0 {}
            break;
        }

        // Read from PTY and write to stdout (non-blocking)
        let mut pty_buf = [0u8; 4096];
        match pty_reader.read(&mut pty_buf) {
            Ok(0) => {
                // EOF means the shell closed
                break;
            }
            Ok(n) => {
                let output = String::from_utf8_lossy(&pty_buf[..n]);
                print!("{}", output);
                let _ = std::io::stdout().flush();
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No data available, check for keyboard input
            }
            Err(e) => {
                log::error!("PTY read error: {e}");
                break;
            }
        }

        // Check for keyboard input
        if !event::poll(std::time::Duration::from_millis(10))
            .map_err(|e| SpellcastError::TerminalPty(format!("Event poll error: {e}")))?
        {
            continue;
        }

        match event::read()
            .map_err(|e| SpellcastError::TerminalPty(format!("Event read error: {e}")))?
        {
            Event::Key(key_event) => {
                // Check kill switch first
                if is_kill_switch(&key_event) {
                    mode_ctrl.toggle_kill_switch();
                    KILL_SWITCH_ENGAGED
                        .store(mode_ctrl.current_mode().is_killed(), Ordering::SeqCst);
                    if mode_ctrl.current_mode().is_killed() {
                        log::info!("Kill switch engaged — Spellcast disabled");
                    } else {
                        log::info!("Kill switch disengaged — Spellcast re-enabled");
                    }
                    continue;
                }

                // In killed mode, pass everything through
                if mode_ctrl.current_mode().is_killed() {
                    write_pty(&mut pty_writer, &key_event)?;
                    continue;
                }

                match mode_ctrl.current_mode() {
                    Mode::Dictation => {
                        // Check for mode toggle (F10)
                        if is_caps_lock_toggle(&key_event) {
                            mode_ctrl.toggle_mode();
                            log::info!("Switched to raw mode");
                            continue;
                        }
                        handle_dictation_key(
                            &key_event,
                            &mut pty_writer,
                            &mut current_tokens,
                            &mut token_index,
                            &mut pending_text,
                            &mut predictions,
                            &tokenizer,
                            config,
                        )?;
                    }
                    Mode::Raw => {
                        // Check for Caps Lock toggle
                        if is_caps_lock_toggle(&key_event) {
                            mode_ctrl.toggle_mode();
                            log::info!("Switched to dictation mode");
                            continue;
                        }
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

    Ok(())
}

/// Render the status bar showing mode, token, and predictions.
fn render_status_bar(
    mode: Mode,
    tokens: &TokenStream,
    token_index: Option<usize>,
    predictions: &[String],
) -> SpellcastResult<()> {
    let mode_str = mode.to_string();

    // Build the status line
    let mut status = format!("[{}]", mode_str);

    // Show current token
    if let Some(idx) = token_index
        && let Some(token) = tokens.get(idx)
    {
        status.push_str(&format!(" @{}:'{}'", idx, token.text));
    }

    // Show predictions
    for (i, pred) in predictions.iter().enumerate() {
        status.push_str(&format!(" {0}:{1}", i + 1, pred));
    }

    // Clear the current line, move to bottom, write status
    let (cols, _rows) = crossterm::terminal::size()
        .map_err(|e| SpellcastError::TerminalRender(format!("Failed to get terminal size: {e}")))?;

    let status_len = status.len() as u16;
    let padding = cols.saturating_sub(status_len);

    // Use reverse video for the status bar
    let padded_status = format!(
        "\r{}\r\x1b[7m{:padding$}\x1b[0m\r",
        "\x1b[K",
        status,
        padding = padding as usize
    );

    // Save cursor, move to bottom, write status, restore cursor
    // This uses the "scroll bottom" approach via ANSI escape
    let (_, rows) = crossterm::terminal::size()
        .map_err(|e| SpellcastError::TerminalRender(format!("Failed to get terminal size: {e}")))?;

    let bottom_row = rows - 1;
    print!("\x1b[s\x1b[{};1H{}", bottom_row + 1, padded_status);
    print!("\x1b[u");
    std::io::stdout().flush().ok();

    Ok(())
}

/// Handle a key event in dictation mode.
#[allow(clippy::too_many_arguments)]
fn handle_dictation_key(
    key: &KeyEvent,
    pty: &mut Box<dyn Write + Send>,
    tokens: &mut TokenStream,
    token_index: &mut Option<usize>,
    pending_text: &mut String,
    predictions: &mut Vec<String>,
    _tokenizer: &HeuristicTokenizer,
    _config: &SpellcastConfig,
) -> SpellcastResult<()> {
    match key.code {
        KeyCode::Char('h') | KeyCode::Left if key.modifiers.is_empty() => {
            // Navigate to previous token
            let idx = token_index.unwrap_or(tokens.len().saturating_sub(1));
            if idx > 0 {
                *token_index = Some(idx - 1);
            }
        }
        KeyCode::Char('l') | KeyCode::Right if key.modifiers.is_empty() => {
            // Navigate to next token
            let idx = token_index.unwrap_or(0);
            if idx + 1 < tokens.len() {
                *token_index = Some(idx + 1);
            }
        }
        KeyCode::Char('x') if key.modifiers.is_empty() => {
            // Delete highlighted token
            if let Some(idx) = *token_index {
                tokens.remove(idx);
                if idx >= tokens.len() {
                    *token_index = if tokens.is_empty() {
                        None
                    } else {
                        Some(tokens.len() - 1)
                    };
                }
            }
            *token_index = None;
        }
        KeyCode::Char('r') if key.modifiers.is_empty() => {
            // Re-dictate: delete highlighted token and prepare for new input
            if let Some(idx) = *token_index {
                tokens.remove(idx);
                *token_index = None;
                *pending_text = String::new();
                // TODO: trigger audio capture + ASR
                log::info!("Re-dictate triggered at token {idx}");
            }
        }
        KeyCode::Char('e') if key.modifiers.is_empty() => {
            // Explain: trigger the explain pipeline
            if let Some(idx) = *token_index
                && let Some(token) = tokens.get(idx)
            {
                log::info!("Explain triggered on token '{}'", token.text);
            }
            // TODO: trigger audio capture + explainer
        }
        KeyCode::Char(c) if ('1'..='3').contains(&c) => {
            // Accept prediction
            let pred_idx = (c as u8 - b'1') as usize;
            if pred_idx < predictions.len() {
                let prediction = predictions[pred_idx].clone();
                if let Some(idx) = *token_index {
                    if idx < tokens.len() {
                        // Replace highlighted token with prediction
                        // For MVP: just write the prediction to the PTY
                        let _ = write!(pty, "{}", prediction);
                        *token_index = None;
                        predictions.clear();
                    }
                } else {
                    let _ = write!(pty, "{}", prediction);
                    predictions.clear();
                }
            }
        }
        KeyCode::Char(' ') => {
            let _ = write!(pty, " ");
        }
        KeyCode::Enter => {
            let _ = writeln!(pty);
        }
        KeyCode::Backspace => {
            let _ = write!(pty, "\x08 \x08");
        }
        KeyCode::Esc => {
            // Exit dictation mode
            *token_index = None;
            predictions.clear();
            // TODO: switch mode via Ctrl key
        }
        _ => {
            // In dictation mode, pass printable characters through
            if let KeyCode::Char(c) = key.code {
                let _ = write!(pty, "{}", c);
            }
        }
    }

    Ok(())
}

/// Check if a key event is the kill switch (Ctrl+Alt+X).
fn is_kill_switch(key: &KeyEvent) -> bool {
    key.code == KeyCode::Char('x')
        && key.modifiers.contains(KeyModifiers::CONTROL)
        && key.modifiers.contains(KeyModifiers::ALT)
}

/// Check if a key event is the Caps Lock toggle.
/// Requires PushKeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES to be active.
fn is_caps_lock_toggle(key: &KeyEvent) -> bool {
    key.code == KeyCode::CapsLock
}

/// Write a key event to the PTY.
fn write_pty(pty: &mut Box<dyn Write + Send>, key: &KeyEvent) -> SpellcastResult<()> {
    match key.code {
        KeyCode::Char(c) => {
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            pty.write_all(s.as_bytes())
                .map_err(|e| SpellcastError::TerminalPty(format!("PTY write error: {e}")))?;
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
        _ => {
            // Unsupported key — ignore
        }
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
            code: KeyCode::Char('x'),
            modifiers: KeyModifiers::CONTROL | KeyModifiers::ALT,
            kind: event::KeyEventKind::Press,
            state: event::KeyEventState::NONE,
        };
        assert!(is_kill_switch(&ks));

        let not_ks = KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::CONTROL,
            kind: event::KeyEventKind::Press,
            state: event::KeyEventState::NONE,
        };
        assert!(!is_kill_switch(&not_ks));
    }

    #[test]
    fn test_get_terminal_size() {
        let result = get_terminal_size();
        // May fail in non-terminal contexts (CI, test runner)
        if let Ok(size) = result {
            assert!(size.cols > 0);
            assert!(size.rows > 0);
        }
    }
}
