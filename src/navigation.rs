// SPDX-License-Identifier: Apache-2.0

//! Advanced token navigation (Vim-style).
//!
//! Provides navigation commands for moving through a token stream.
//! Supports count prefixes, visual selection, and fuzzy search.

use crate::tokenizer::{Token, TokenStream, TokenType};

/// Direction for navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavDirection {
    Forward,
    Backward,
}

/// Navigation action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavAction {
    PrevToken,
    NextToken,
    PrevLine,
    NextLine,
    WordForward,
    WordBackward,
    EndOfToken,
    PrevParagraph,
    NextParagraph,
    FirstToken,
    LastToken,
    FirstInLine,
    LastInLine,
    FindForward,
    FindBackward,
}

/// Visual selection mode state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualMode {
    Inactive,
    CharWise,
    LineWise,
}

/// The navigation state for a token stream.
#[derive(Debug, Clone)]
pub struct NavigationState {
    /// Current token index.
    pub current_index: usize,
    /// Visual mode state.
    pub visual_mode: VisualMode,
    /// Selection anchor index (start of visual selection).
    pub visual_anchor: Option<usize>,
    /// Active count prefix (0 means no prefix).
    pub count_prefix: usize,
}

impl NavigationState {
    pub fn new() -> Self {
        Self {
            current_index: 0,
            visual_mode: VisualMode::Inactive,
            visual_anchor: None,
            count_prefix: 0,
        }
    }

    /// Get the effective count (prefix or 1).
    fn count(&self) -> usize {
        if self.count_prefix > 0 {
            self.count_prefix
        } else {
            1
        }
    }

    /// Reset count prefix after applying.
    pub fn reset_count(&mut self) {
        self.count_prefix = 0;
    }

    /// Navigate the token stream based on an action.
    pub fn navigate(&mut self, stream: &TokenStream, action: NavAction) -> usize {
        let count = self.count();
        self.reset_count();
        let len = stream.tokens.len();
        if len == 0 {
            return self.current_index;
        }

        match action {
            NavAction::PrevToken => {
                self.current_index = self.current_index.saturating_sub(count);
            }
            NavAction::NextToken => {
                self.current_index = (self.current_index + count).min(len.saturating_sub(1));
            }
            NavAction::PrevLine => {
                self.current_index = self.go_to_prev_line(stream);
            }
            NavAction::NextLine => {
                self.current_index = self.go_to_next_line(stream);
            }
            NavAction::WordForward => {
                self.current_index = self.skip_to_word(stream, NavDirection::Forward, count);
            }
            NavAction::WordBackward => {
                self.current_index = self.skip_to_word(stream, NavDirection::Backward, count);
            }
            NavAction::EndOfToken => {
                // Stay on current token, but mark end
                // (For future: move cursor to end of token text)
            }
            NavAction::PrevParagraph => {
                self.current_index = self.go_to_paragraph(stream, NavDirection::Backward, count);
            }
            NavAction::NextParagraph => {
                self.current_index = self.go_to_paragraph(stream, NavDirection::Forward, count);
            }
            NavAction::FirstToken => {
                self.current_index = 0;
            }
            NavAction::LastToken => {
                self.current_index = len.saturating_sub(1);
            }
            NavAction::FirstInLine => {
                self.current_index = self.go_to_first_in_line(stream);
            }
            NavAction::LastInLine => {
                self.current_index = self.go_to_last_in_line(stream);
            }
            NavAction::FindForward | NavAction::FindBackward => {
                // Handled externally via fuzzy search
            }
        }

        self.current_index
    }

    /// Toggle visual mode.
    pub fn toggle_visual(&mut self) {
        match self.visual_mode {
            VisualMode::Inactive => {
                self.visual_mode = VisualMode::CharWise;
                self.visual_anchor = Some(self.current_index);
            }
            _ => {
                self.visual_mode = VisualMode::Inactive;
                self.visual_anchor = None;
            }
        }
    }

    /// Set line-wise visual mode.
    pub fn set_visual_line(&mut self) {
        self.visual_mode = VisualMode::LineWise;
        self.visual_anchor = Some(self.current_index);
    }

    /// Get the selected range (if any).
    pub fn selected_range(&self) -> Option<(usize, usize)> {
        let anchor = self.visual_anchor?;
        match self.visual_mode {
            VisualMode::Inactive => None,
            _ => {
                let start = anchor.min(self.current_index);
                let end = anchor.max(self.current_index);
                Some((start, end))
            }
        }
    }

    /// Get selected tokens from the stream.
    pub fn selected_tokens<'a>(&self, stream: &'a TokenStream) -> Option<&'a [Token]> {
        let (start, end) = self.selected_range()?;
        if end < stream.tokens.len() {
            Some(&stream.tokens[start..=end])
        } else {
            Some(&stream.tokens[start..])
        }
    }

    // --- Internal helpers ---

    /// Find the first token of the next/previous line.
    fn go_to_prev_line(&self, stream: &TokenStream) -> usize {
        let tokens = &stream.tokens;
        let current = self.current_index;

        // Find the start of the current line
        let current_line_start = self.line_start(tokens, current);
        if current_line_start == 0 {
            return 0; // Already at first line
        }

        // Find the start of the previous line
        self.line_start(tokens, current_line_start.saturating_sub(1))
    }

    fn go_to_next_line(&self, stream: &TokenStream) -> usize {
        let tokens = &stream.tokens;
        let current = self.current_index;

        // Find the end of the current line
        let current_line_end = self.line_end(tokens, current);
        if current_line_end >= tokens.len().saturating_sub(1) {
            return tokens.len().saturating_sub(1);
        }

        // First token of the next line
        current_line_end + 1
    }

    /// Find the first token of the line containing `index`.
    fn line_start(&self, tokens: &[Token], index: usize) -> usize {
        let mut i = index;
        // If current token is a newline, we're between lines — find start of next line
        if tokens[i].token_type == TokenType::Whitespace
            && tokens[i].text.contains('\n')
            && i + 1 < tokens.len()
        {
            return i + 1;
        }
        while i > 0 {
            if tokens[i].token_type == TokenType::Whitespace
                && tokens[i].text.contains('\n')
            {
                return if i + 1 < tokens.len() { i + 1 } else { i };
            }
            i -= 1;
        }
        0
    }

    /// Find the last token of the line containing `index`.
    fn line_end(&self, tokens: &[Token], index: usize) -> usize {
        let mut i = index;
        while i + 1 < tokens.len() {
            if tokens[i].token_type == TokenType::Whitespace
                && tokens[i].text.contains('\n')
            {
                return i.saturating_sub(1);
            }
            i += 1;
        }
        tokens.len().saturating_sub(1)
    }

    /// Skip to the next/previous word-like token (skip punctuation/whitespace).
    fn skip_to_word(&self, stream: &TokenStream, dir: NavDirection, count: usize) -> usize {
        let tokens = &stream.tokens;
        let mut idx = self.current_index;
        let mut remaining = count;

        match dir {
            NavDirection::Forward => {
                while remaining > 0 && idx + 1 < tokens.len() {
                    idx += 1;
                    if Self::is_word_like(&tokens[idx]) {
                        remaining -= 1;
                    }
                }
            }
            NavDirection::Backward => {
                while remaining > 0 && idx > 0 {
                    idx -= 1;
                    if Self::is_word_like(&tokens[idx]) {
                        remaining -= 1;
                    }
                }
            }
        }
        idx
    }

    /// Check if a token is "word-like" (not pure punctuation/whitespace).
    fn is_word_like(t: &Token) -> bool {
        !matches!(
            t.token_type,
            TokenType::Punctuation | TokenType::Whitespace | TokenType::Operator
        )
    }

    /// Navigate by paragraph (double newline) or code block boundaries.
    fn go_to_paragraph(&self, stream: &TokenStream, dir: NavDirection, count: usize) -> usize {
        let tokens = &stream.tokens;
        let mut idx = self.current_index;
        let mut remaining = count;

        match dir {
            NavDirection::Forward => {
                while remaining > 0 && idx + 1 < tokens.len() {
                    idx += 1;
                    if Self::is_paragraph_boundary(tokens, idx) {
                        remaining = remaining.saturating_sub(1);
                        // Always advance past the boundary token
                        if idx + 1 < tokens.len() {
                            idx += 1;
                        }
                    }
                }
            }
            NavDirection::Backward => {
                while remaining > 0 && idx > 0 {
                    idx = idx.saturating_sub(1);
                    if Self::is_paragraph_boundary(tokens, idx) {
                        remaining = remaining.saturating_sub(1);
                        // Always move back past the boundary
                        if idx > 0 {
                            idx = idx.saturating_sub(1);
                        }
                    }
                }
            }
        }
        idx
    }

    /// Check if a token position is a paragraph boundary (double newline).
    fn is_paragraph_boundary(tokens: &[Token], idx: usize) -> bool {
        if tokens[idx].token_type == TokenType::Whitespace
            && tokens[idx].text.matches('\n').count() >= 2
        {
            return true;
        }
        false
    }

    /// Go to the first token on the current line.
    fn go_to_first_in_line(&self, stream: &TokenStream) -> usize {
        self.line_start(&stream.tokens, self.current_index)
    }

    /// Go to the last token on the current line.
    fn go_to_last_in_line(&self, stream: &TokenStream) -> usize {
        self.line_end(&stream.tokens, self.current_index)
    }
}

impl Default for NavigationState {
    fn default() -> Self {
        Self::new()
    }
}

/// Fuzzy token search: find tokens matching a spoken query.
pub struct FuzzySearcher {
    /// Last query for n/N navigation.
    last_query: Option<String>,
    /// Match indices from the last search.
    last_matches: Vec<usize>,
    /// Current position in the match list.
    match_index: usize,
}

impl FuzzySearcher {
    pub fn new() -> Self {
        Self {
            last_query: None,
            last_matches: Vec::new(),
            match_index: 0,
        }
    }

    /// Search for tokens matching a spoken query (fuzzy: phoneme similarity).
    pub fn search(&mut self, stream: &TokenStream, query: &str) -> Vec<usize> {
        let query = query.to_lowercase();
        let matches: Vec<usize> = stream
            .tokens
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                let lower = t.text.to_lowercase();
                lower.contains(&query) || Self::fuzzy_match(&lower, &query)
            })
            .map(|(i, _)| i)
            .collect();

        self.last_query = Some(query);
        self.last_matches = matches.clone();
        self.match_index = 0;
        matches
    }

    /// Jump to the next match (n).
    pub fn next_match(&mut self) -> Option<usize> {
        if self.last_matches.is_empty() {
            return None;
        }
        self.match_index = (self.match_index + 1) % self.last_matches.len();
        Some(self.last_matches[self.match_index])
    }

    /// Jump to the previous match (N).
    pub fn prev_match(&mut self) -> Option<usize> {
        if self.last_matches.is_empty() {
            return None;
        }
        self.match_index = if self.match_index == 0 {
            self.last_matches.len().saturating_sub(1)
        } else {
            self.match_index - 1
        };
        Some(self.last_matches[self.match_index])
    }

    /// Simple fuzzy match: check if characters of `query` appear in order in `text`.
    fn fuzzy_match(text: &str, query: &str) -> bool {
        let mut qi = query.chars().peekable();
        for c in text.chars() {
            if qi.peek() == Some(&c) {
                qi.next();
                if qi.peek().is_none() {
                    return true;
                }
            }
        }
        false
    }
}

impl Default for FuzzySearcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::{Token, TokenStream, TokenContext};

    fn make_stream(tokens: Vec<(&str, TokenType)>) -> TokenStream {
        let mut pos = 0;
        TokenStream {
            tokens: tokens
                .into_iter()
                .map(|(text, token_type)| {
                    let t = Token {
                        text: text.to_string(),
                        offset: pos,
                        length: text.len(),
                        token_type,
                    };
                    pos += text.len();
                    t
                })
                .collect(),
            context: TokenContext::Prose,
        }
    }

    #[test]
    fn test_prev_token() {
        let mut nav = NavigationState::new();
        nav.current_index = 2;
        let stream = make_stream(vec![("a", TokenType::Word), ("b", TokenType::Word), ("c", TokenType::Word)]);
        nav.navigate(&stream, NavAction::PrevToken);
        assert_eq!(nav.current_index, 1);
    }

    #[test]
    fn test_next_token() {
        let mut nav = NavigationState::new();
        nav.current_index = 0;
        let stream = make_stream(vec![
            ("a", TokenType::Word),
            ("b", TokenType::Word),
            ("c", TokenType::Word),
        ]);
        nav.navigate(&stream, NavAction::NextToken);
        assert_eq!(nav.current_index, 1);
    }

    #[test]
    fn test_count_prefix() {
        let mut nav = NavigationState::new();
        nav.count_prefix = 3;
        nav.current_index = 0;
        let stream = make_stream(vec![
            ("a", TokenType::Word),
            ("b", TokenType::Word),
            ("c", TokenType::Word),
            ("d", TokenType::Word),
            ("e", TokenType::Word),
        ]);
        nav.navigate(&stream, NavAction::NextToken);
        assert_eq!(nav.current_index, 3);
        assert_eq!(nav.count_prefix, 0);
    }

    #[test]
    fn test_first_and_last_token() {
        let mut nav = NavigationState::new();
        let stream = make_stream(vec![
            ("a", TokenType::Word),
            ("b", TokenType::Word),
            ("c", TokenType::Word),
        ]);

        nav.navigate(&stream, NavAction::FirstToken);
        assert_eq!(nav.current_index, 0);

        nav.navigate(&stream, NavAction::LastToken);
        assert_eq!(nav.current_index, 2);
    }

    #[test]
    fn test_word_forward_skips_punctuation() {
        let mut nav = NavigationState::new();
        nav.current_index = 0;
        let stream = make_stream(vec![
            ("hello", TokenType::Word),
            (",", TokenType::Punctuation),
            (" ", TokenType::Whitespace),
            ("world", TokenType::Word),
        ]);
        nav.navigate(&stream, NavAction::WordForward);
        assert_eq!(nav.current_index, 3);
    }

    #[test]
    fn test_visual_mode_toggle() {
        let mut nav = NavigationState::new();
        assert_eq!(nav.visual_mode, VisualMode::Inactive);
        nav.toggle_visual();
        assert_eq!(nav.visual_mode, VisualMode::CharWise);
        assert_eq!(nav.visual_anchor, Some(0));
        nav.toggle_visual();
        assert_eq!(nav.visual_mode, VisualMode::Inactive);
    }

    #[test]
    fn test_visual_selection_range() {
        let mut nav = NavigationState::new();
        nav.current_index = 0;
        nav.toggle_visual();
        nav.current_index = 3;
        let range = nav.selected_range();
        assert_eq!(range, Some((0, 3)));
    }

    #[test]
    fn test_fuzzy_search() {
        let mut searcher = FuzzySearcher::new();
        let stream = make_stream(vec![
            ("hello", TokenType::Word),
            ("world", TokenType::Word),
            ("help", TokenType::Word),
            ("helm", TokenType::Word),
        ]);
        let matches = searcher.search(&stream, "hel");
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn test_fuzzy_search_no_matches() {
        let mut searcher = FuzzySearcher::new();
        let stream = make_stream(vec![
            ("alpha", TokenType::Word),
            ("beta", TokenType::Word),
        ]);
        let matches = searcher.search(&stream, "zzz");
        assert!(matches.is_empty());
    }

    #[test]
    fn test_next_prev_match() {
        let mut searcher = FuzzySearcher::new();
        let stream = make_stream(vec![
            ("a", TokenType::Word),
            ("b", TokenType::Word),
            ("a", TokenType::Word),
            ("c", TokenType::Word),
        ]);
        searcher.search(&stream, "a");
        assert_eq!(searcher.next_match(), Some(2));
        assert_eq!(searcher.next_match(), Some(0)); // wraps
        assert_eq!(searcher.prev_match(), Some(2));
    }

    #[test]
    fn test_paragraph_navigation() {
        let mut nav = NavigationState::new();
        nav.current_index = 0;
        let stream = make_stream(vec![
            ("hello", TokenType::Word),
            (" ", TokenType::Whitespace),
            ("world", TokenType::Word),
            ("\n\n", TokenType::Whitespace),
            ("new", TokenType::Word),
            (" ", TokenType::Whitespace),
            ("para", TokenType::Word),
        ]);
        nav.navigate(&stream, NavAction::NextParagraph);
        assert_eq!(nav.current_index, 4);
    }

    #[test]
    fn test_first_in_line() {
        let mut nav = NavigationState::new();
        nav.current_index = 3;
        let stream = make_stream(vec![
            ("first", TokenType::Word),
            (" ", TokenType::Whitespace),
            ("second", TokenType::Word),
            ("\n", TokenType::Whitespace),
            ("third", TokenType::Word),
        ]);
        nav.navigate(&stream, NavAction::FirstInLine);
        assert_eq!(nav.current_index, 4); // First token of line after newline
    }
}