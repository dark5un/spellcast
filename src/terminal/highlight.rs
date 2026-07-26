// SPDX-License-Identifier: Apache-2.0

//! Terminal highlighting — in-body token highlighting with ANSI escapes.
//!
//! Highlights the current token directly in the terminal output using
//! reverse video or underline ANSI escape sequences. Maintains a virtual
//! text buffer to track cursor position and scroll state.
//!
//! Fallback: if the token has scrolled off-screen, show in the status line.

use crate::tokenizer::{Token, TokenType};

/// ANSI escape sequences for terminal highlighting.
pub mod ansi {
    /// Reverse video (swap foreground/background).
    pub const REVERSE: &str = "\x1b[7m";
    /// Underline.
    pub const UNDERLINE: &str = "\x1b[4m";
    /// Bold.
    pub const BOLD: &str = "\x1b[1m";
    /// Reset all attributes.
    pub const RESET: &str = "\x1b[0m";
    /// Save cursor position.
    pub const SAVE: &str = "\x1b[s";
    /// Restore cursor position.
    pub const RESTORE: &str = "\x1b[u";
    /// Move cursor to row, col (1-indexed).
    pub fn goto(row: u16, col: u16) -> String {
        format!("\x1b[{};{}H", row, col)
    }
    /// Move cursor up N rows.
    pub fn up(n: u16) -> String {
        format!("\x1b[{}A", n)
    }
    /// Clear from cursor to end of line.
    pub const CLEAR_LINE: &str = "\x1b[K";
}

/// The highlight engine tracks the terminal state and manages highlighting.
pub struct HighlightEngine {
    /// Terminal dimensions (cols, rows).
    pub term_width: u16,
    pub term_height: u16,
}

impl HighlightEngine {
    pub fn new(term_width: u16, term_height: u16) -> Self {
        Self {
            term_width,
            term_height,
        }
    }

    /// Update terminal dimensions.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.term_width = width;
        self.term_height = height;
    }

    /// Generate ANSI sequences to highlight a token at a given position.
    pub fn highlight_token(
        &self,
        token: &Token,
        row: u16,
        col: u16,
        style: HighlightStyle,
    ) -> String {
        let (start_seq, end_seq) = style.sequences();
        format!(
            "{save}{goto}{start}{text}{end}{restore}",
            save = ansi::SAVE,
            goto = ansi::goto(row, col),
            start = start_seq,
            text = token.text,
            end = end_seq,
            restore = ansi::RESTORE,
        )
    }

    /// Generate ANSI sequences to remove a token highlight.
    #[allow(dead_code)]
    pub fn unhighlight_token(
        &self,
        token: &Token,
        row: u16,
        col: u16,
    ) -> String {
        format!(
            "{save}{goto}{text}{restore}",
            save = ansi::SAVE,
            goto = ansi::goto(row, col),
            text = token.text,
            restore = ansi::RESTORE,
        )
    }

    /// Generate the predictions inline display below a token.
    pub fn inline_predictions(
        &self,
        predictions: &[String],
        row: u16,
    ) -> String {
        if predictions.is_empty() {
            return String::new();
        }

        let pred_line = predictions
            .iter()
            .enumerate()
            .map(|(i, p)| format!(" {}: {}", i + 1, p))
            .collect::<Vec<_>>()
            .join(" ");

        format!(
            "{save}{goto}{clear}{text}{restore}",
            save = ansi::SAVE,
            goto = ansi::goto(row + 1, 1),
            clear = ansi::CLEAR_LINE,
            text = format!("  ╰─ {}", pred_line),
            restore = ansi::RESTORE,
        )
    }

    /// Check if a given row is visible in the terminal.
    pub fn is_visible(&self, row: u16) -> bool {
        row <= self.term_height
    }
}

/// Style of highlighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightStyle {
    Reverse,
    Underline,
    Bold,
}

impl HighlightStyle {
    fn sequences(&self) -> (&'static str, &'static str) {
        match self {
            HighlightStyle::Reverse => (ansi::REVERSE, ansi::RESET),
            HighlightStyle::Underline => (ansi::UNDERLINE, ansi::RESET),
            HighlightStyle::Bold => (ansi::BOLD, ansi::RESET),
        }
    }
}

/// Virtual text buffer that mirrors terminal content.
pub struct VirtualBuffer {
    /// Lines of text currently visible.
    lines: Vec<String>,
    /// Maximum width (terminal columns).
    width: usize,
    /// Current scroll offset (number of lines scrolled up).
    #[allow(dead_code)]
    scroll_offset: usize,
}

impl VirtualBuffer {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            lines: vec![String::new(); height],
            width,
            scroll_offset: 0,
        }
    }

    /// Add text to the buffer (simulating terminal output).
    #[allow(dead_code)]
    pub fn push_text(&mut self, text: &str) {
        for line in text.lines() {
            self.lines.push(line.to_string());
            if self.lines.len() > self.lines.capacity() {
                self.lines.remove(0);
                self.scroll_offset += 1;
            }
        }
    }

    /// Get the row (1-indexed) of a token based on its byte offset.
    #[allow(dead_code)]
    pub fn token_row(&self, _offset: usize) -> Option<u16> {
        let mut total_bytes = 0usize;
        for (i, line) in self.lines.iter().enumerate() {
            total_bytes += line.len() + 1;
            if total_bytes > _offset {
                return Some((i + 1) as u16);
            }
        }
        None
    }

    /// Clear the buffer.
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.lines.clear();
        self.scroll_offset = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_style_sequences() {
        assert_eq!(
            HighlightStyle::Reverse.sequences(),
            ("\x1b[7m", "\x1b[0m")
        );
        assert_eq!(
            HighlightStyle::Underline.sequences(),
            ("\x1b[4m", "\x1b[0m")
        );
    }

    #[test]
    fn test_highlight_token_generates_ansi() {
        let engine = HighlightEngine::new(80, 24);
        let token = Token {
            text: "hello".to_string(),
            offset: 0,
            length: 5,
            token_type: TokenType::Word,
        };
        let result = engine.highlight_token(&token, 5, 10, HighlightStyle::Reverse);
        assert!(result.contains("\x1b[7m"));
        assert!(result.contains("hello"));
        assert!(result.contains("\x1b[0m"));
    }

    #[test]
    fn test_unhighlight_token() {
        let engine = HighlightEngine::new(80, 24);
        let token = Token {
            text: "world".to_string(),
            offset: 6,
            length: 5,
            token_type: TokenType::Word,
        };
        let result = engine.unhighlight_token(&token, 3, 5);
        assert!(result.contains("world"));
        assert!(!result.contains("\x1b[7m"));
    }

    #[test]
    fn test_inline_predictions() {
        let engine = HighlightEngine::new(80, 24);
        let predictions = vec!["box".to_string(), "fog".to_string(), "sock".to_string()];
        let result = engine.inline_predictions(&predictions, 10);
        assert!(result.contains("1: box"));
        assert!(result.contains("2: fog"));
        assert!(result.contains("3: sock"));
    }

    #[test]
    fn test_inline_predictions_empty() {
        let engine = HighlightEngine::new(80, 24);
        assert!(engine.inline_predictions(&[], 10).is_empty());
    }

    #[test]
    fn test_visible_true() {
        let engine = HighlightEngine::new(80, 24);
        assert!(engine.is_visible(20));
    }

    #[test]
    fn test_visible_false() {
        let engine = HighlightEngine::new(80, 24);
        assert!(!engine.is_visible(30));
    }

    #[test]
    fn test_virtual_buffer_push_text() {
        let mut buf = VirtualBuffer::new(80, 24);
        buf.push_text("hello world\nsecond line\nthird line");
        assert_eq!(buf.lines.len(), 27);
    }

    #[test]
    fn test_highlight_engine_resize() {
        let mut engine = HighlightEngine::new(80, 24);
        engine.resize(120, 30);
        assert_eq!(engine.term_width, 120);
        assert_eq!(engine.term_height, 30);
    }
}