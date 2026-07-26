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
use std::sync::mpsc;

pub mod highlight;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
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
    _memory: &MemoryStore,
    shell: Option<&str>,
) -> SpellcastResult<()> {
    // Raw mode FIRST, then alternate screen
    terminal::enable_raw_mode()
        .map_err(|e| SpellcastError::TerminalRender(format!("Failed to enable raw mode: {e}")))?;
    crossterm::execute!(std::io::stdout(), EnterAlternateScreen).map_err(|e| {
        SpellcastError::TerminalRender(format!("Failed to enter alternate screen: {e}"))
    })?;
    let _guard = TerminalGuard::new();

    let result = run_inner(config, mode_ctrl, _memory, shell);

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

    // PTY reader — spawn a dedicated thread so it doesn't block the event loop
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
    let mut pending_text: String = String::new();
    let mut predictions: Vec<String> = Vec::new();

    // Main event loop
    loop {
        // Check if the child process is still alive
        if let Ok(Some(_)) = child.try_wait() {
            log::info!("Shell process exited");
            break;
        }

        // Drain PTY output (non-blocking from channel)
        while let Ok(data) = pty_rx.try_recv() {
            let output = String::from_utf8_lossy(&data);
            print!("{}", output);
        }
        let _ = std::io::stdout().flush();

        // Render status bar
        render_status_bar(
            mode_ctrl.current_mode(),
            &current_tokens,
            token_index,
            &predictions,
        )?;

        // Poll for keyboard input (10ms timeout)
        if !event::poll(std::time::Duration::from_millis(50))
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
                        if is_mode_toggle(&key_event) {
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
                        if is_mode_toggle(&key_event) {
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
    let mut status = format!("[{}]", mode_str);

    if let Some(idx) = token_index
        && let Some(token) = tokens.get(idx)
    {
        status.push_str(&format!(" @{}:'{}'", idx, token.text));
    }

    for (i, pred) in predictions.iter().enumerate() {
        status.push_str(&format!(" {0}:{1}", i + 1, pred));
    }

    let (cols, rows) = crossterm::terminal::size()
        .map_err(|e| SpellcastError::TerminalRender(format!("Failed to get terminal size: {e}")))?;

    let status_len = status.len() as u16;
    let padding = cols.saturating_sub(status_len);

    let padded_status = format!(
        "\r{}\r\x1b[7m{:padding$}\x1b[0m\r",
        "\x1b[K",
        status,
        padding = padding as usize
    );

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
            let idx = token_index.unwrap_or(tokens.len().saturating_sub(1));
            if idx > 0 {
                *token_index = Some(idx - 1);
            }
        }
        KeyCode::Char('l') | KeyCode::Right if key.modifiers.is_empty() => {
            let idx = token_index.unwrap_or(0);
            if idx + 1 < tokens.len() {
                *token_index = Some(idx + 1);
            }
        }
        KeyCode::Char('x') if key.modifiers.is_empty() => {
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
            if let Some(idx) = *token_index {
                tokens.remove(idx);
                *token_index = None;
                *pending_text = String::new();
                log::info!("Re-dictate triggered at token {idx}");
            }
        }
        KeyCode::Char('e') if key.modifiers.is_empty() => {
            if let Some(idx) = *token_index
                && let Some(token) = tokens.get(idx)
            {
                log::info!("Explain triggered on token '{}'", token.text);
            }
        }
        KeyCode::Char(c) if ('1'..='3').contains(&c) => {
            let pred_idx = (c as u8 - b'1') as usize;
            if pred_idx < predictions.len() {
                let prediction = predictions[pred_idx].clone();
                if let Some(idx) = *token_index {
                    if idx < tokens.len() {
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
            *token_index = None;
            predictions.clear();
        }
        _ => {
            if let KeyCode::Char(c) = key.code {
                let _ = write!(pty, "{}", c);
            }
        }
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
    // Caps Lock via kitty keyboard protocol (if terminal supports it)
    if key.code == KeyCode::CapsLock {
        return true;
    }
    // Ctrl+Space — works on ALL terminals, no kitty protocol needed
    if key.code == KeyCode::Char(' ') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return true;
    }
    false
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
        // Ctrl+G with CONTROL modifier (kitty protocol)
        let ks = KeyEvent {
            code: KeyCode::Char('g'),
            modifiers: KeyModifiers::CONTROL,
            kind: event::KeyEventKind::Press,
            state: event::KeyEventState::NONE,
        };
        assert!(is_kill_switch(&ks));

        // Raw BEL character (0x07) — what Ctrl+G sends in raw mode
        let bel_ks = KeyEvent {
            code: KeyCode::Char('\x07'),
            modifiers: KeyModifiers::NONE,
            kind: event::KeyEventKind::Press,
            state: event::KeyEventState::NONE,
        };
        assert!(is_kill_switch(&bel_ks));

        // Not a kill switch
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
    fn test_get_terminal_size() {
        let result = get_terminal_size();
        if let Ok(size) = result {
            assert!(size.cols > 0);
            assert!(size.rows > 0);
        }
    }
}
