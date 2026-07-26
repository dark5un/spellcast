// SPDX-License-Identifier: Apache-2.0

//! Code spelling modes that transform dictated text into naming conventions.
//!
//! Modes are sticky (active until turned off) or one-shot (apply to next utterance).
//! Supported modes: snake, camel, pascal, kebab, screaming_snake, single_word, spelling.

/// Naming convention modes for transforming dictated text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpellingMode {
    /// snake_case (default for code contexts)
    Snake,
    /// camelCase
    Camel,
    /// PascalCase
    Pascal,
    /// kebab-case
    Kebab,
    /// SCREAMING_SNAKE_CASE
    ScreamingSnake,
    /// foobarbaz (no separators)
    SingleWord,
    /// NATO phonetic alphabet → single characters
    Spelling,
    /// No transformation (passthrough)
    None,
}

impl std::fmt::Display for SpellingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpellingMode::Snake => write!(f, "snake"),
            SpellingMode::Camel => write!(f, "camel"),
            SpellingMode::Pascal => write!(f, "pascal"),
            SpellingMode::Kebab => write!(f, "kebab"),
            SpellingMode::ScreamingSnake => write!(f, "screaming"),
            SpellingMode::SingleWord => write!(f, "single"),
            SpellingMode::Spelling => write!(f, "spelling"),
            SpellingMode::None => write!(f, "none"),
        }
    }
}

/// The state of spelling mode for the current session.
#[derive(Debug, Clone)]
pub struct SpellingModeState {
    /// Active mode.
    pub mode: SpellingMode,
    /// If true, the mode applies to the next utterance only then resets to None.
    pub one_shot: bool,
}

impl SpellingModeState {
    /// Create a new state with no active mode.
    pub fn new() -> Self {
        Self {
            mode: SpellingMode::None,
            one_shot: false,
        }
    }

    /// Set a sticky mode.
    pub fn set_sticky(&mut self, mode: SpellingMode) {
        self.mode = mode;
        self.one_shot = false;
    }

    /// Set a one-shot mode.
    pub fn set_one_shot(&mut self, mode: SpellingMode) {
        self.mode = mode;
        self.one_shot = true;
    }

    /// Reset to no mode.
    pub fn reset(&mut self) {
        self.mode = SpellingMode::None;
        self.one_shot = false;
    }

    /// Is a mode currently active?
    pub fn is_active(&self) -> bool {
        self.mode != SpellingMode::None
    }

    /// Apply the active mode to the given text, consuming one-shot if set.
    /// Returns the transformed text and whether a mode was applied.
    pub fn apply(&mut self, text: &str) -> String {
        if self.mode == SpellingMode::None {
            return text.to_string();
        }

        let result = apply_mode(text, self.mode);

        if self.one_shot {
            self.reset();
        }

        result
    }
}

impl Default for SpellingModeState {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a verbal command string into a SpellingMode.
pub fn parse_spelling_command(command: &str) -> Option<SpellingMode> {
    match command.to_lowercase().trim() {
        "snake" | "snake case" | "underscore" => Some(SpellingMode::Snake),
        "camel" | "camel case" | "lower camel" => Some(SpellingMode::Camel),
        "pascal" | "pascal case" | "upper camel" => Some(SpellingMode::Pascal),
        "kebab" | "kebab case" | "hyphen" | "dash" => Some(SpellingMode::Kebab),
        "screaming" | "screaming case" | "screaming snake" => Some(SpellingMode::ScreamingSnake),
        "single" | "single word" | "compound" => Some(SpellingMode::SingleWord),
        "spelling" | "spell" | "nato" | "alpha bravo" => Some(SpellingMode::Spelling),
        _ => None,
    }
}

/// Apply a naming convention to multi-word text.
pub fn apply_mode(text: &str, mode: SpellingMode) -> String {
    // Split into words
    let words: Vec<&str> = text
        .split(|c: char| c.is_ascii_whitespace() || c == '_' || c == '-')
        .filter(|w| !w.is_empty())
        .collect();

    if words.is_empty() {
        return text.to_string();
    }

    match mode {
        SpellingMode::Snake => words.join("_").to_lowercase(),
        SpellingMode::Camel => {
            let mut result = words[0].to_lowercase();
            for w in &words[1..] {
                let mut chars = w.chars();
                if let Some(first) = chars.next() {
                    result.push(first.to_ascii_uppercase());
                    result.push_str(&chars.as_str().to_lowercase());
                }
            }
            result
        }
        SpellingMode::Pascal => {
            let mut result = String::new();
            for w in &words {
                let mut chars = w.chars();
                if let Some(first) = chars.next() {
                    result.push(first.to_ascii_uppercase());
                    result.push_str(&chars.as_str().to_lowercase());
                }
            }
            result
        }
        SpellingMode::Kebab => words.join("-").to_lowercase(),
        SpellingMode::ScreamingSnake => words.join("_").to_uppercase(),
        SpellingMode::SingleWord => words.join("").to_lowercase(),
        SpellingMode::Spelling => {
            // NATO phonetic alphabet → single characters
            let mut result = String::new();
            for w in &words {
                if let Some(ch) = nato_to_char(w) {
                    result.push(ch);
                }
            }
            result
        }
        SpellingMode::None => text.to_string(),
    }
}

/// Map NATO phonetic alphabet words to characters.
fn nato_to_char(word: &str) -> Option<char> {
    match word.to_lowercase().trim() {
        "alpha" | "alfa" => Some('a'),
        "bravo" => Some('b'),
        "charlie" => Some('c'),
        "delta" => Some('d'),
        "echo" => Some('e'),
        "foxtrot" => Some('f'),
        "golf" => Some('g'),
        "hotel" => Some('h'),
        "india" => Some('i'),
        "juliett" | "juliet" => Some('j'),
        "kilo" => Some('k'),
        "lima" => Some('l'),
        "mike" => Some('m'),
        "november" => Some('n'),
        "oscar" => Some('o'),
        "papa" => Some('p'),
        "quebec" => Some('q'),
        "romeo" => Some('r'),
        "sierra" => Some('s'),
        "tango" => Some('t'),
        "uniform" => Some('u'),
        "victor" => Some('v'),
        "whiskey" | "whisky" => Some('w'),
        "x-ray" | "xray" => Some('x'),
        "yankee" => Some('y'),
        "zulu" => Some('z'),
        // Digits
        "zero" => Some('0'),
        "one" => Some('1'),
        "two" => Some('2'),
        "three" => Some('3'),
        "four" => Some('4'),
        "five" => Some('5'),
        "six" => Some('6'),
        "seven" => Some('7'),
        "eight" => Some('8'),
        "nine" => Some('9'),
        _ => {
            // If it's a single character or not in NATO, return the first char
            word.chars().next()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snake_case() {
        assert_eq!(
            apply_mode("foo bar baz", SpellingMode::Snake),
            "foo_bar_baz"
        );
    }

    #[test]
    fn test_camel_case() {
        assert_eq!(apply_mode("foo bar baz", SpellingMode::Camel), "fooBarBaz");
    }

    #[test]
    fn test_pascal_case() {
        assert_eq!(apply_mode("foo bar baz", SpellingMode::Pascal), "FooBarBaz");
    }

    #[test]
    fn test_kebab_case() {
        assert_eq!(
            apply_mode("foo bar baz", SpellingMode::Kebab),
            "foo-bar-baz"
        );
    }

    #[test]
    fn test_screaming_snake() {
        assert_eq!(
            apply_mode("foo bar baz", SpellingMode::ScreamingSnake),
            "FOO_BAR_BAZ"
        );
    }

    #[test]
    fn test_single_word() {
        assert_eq!(
            apply_mode("foo bar baz", SpellingMode::SingleWord),
            "foobarbaz"
        );
    }

    #[test]
    fn test_spelling_nato() {
        assert_eq!(
            apply_mode("alpha bravo charlie", SpellingMode::Spelling),
            "abc"
        );
    }

    #[test]
    fn test_spelling_full_alphabet() {
        let input =
            "alpha bravo charlie delta echo foxtrot golf hotel india juliett kilo lima mike";
        assert_eq!(apply_mode(input, SpellingMode::Spelling), "abcdefghijklm");
    }

    #[test]
    fn test_none_mode() {
        assert_eq!(apply_mode("hello world", SpellingMode::None), "hello world");
    }

    #[test]
    fn test_parse_spelling_commands() {
        assert_eq!(parse_spelling_command("snake"), Some(SpellingMode::Snake));
        assert_eq!(parse_spelling_command("camel"), Some(SpellingMode::Camel));
        assert_eq!(parse_spelling_command("pascal"), Some(SpellingMode::Pascal));
        assert_eq!(parse_spelling_command("kebab"), Some(SpellingMode::Kebab));
        assert_eq!(
            parse_spelling_command("spelling"),
            Some(SpellingMode::Spelling)
        );
        assert_eq!(parse_spelling_command("unknown"), None);
    }

    #[test]
    fn test_mode_state_sticky() {
        let mut state = SpellingModeState::new();
        state.set_sticky(SpellingMode::Snake);
        assert!(state.is_active());

        let r1 = state.apply("hello world");
        assert_eq!(r1, "hello_world");
        // Sticky: should still be active after one apply
        assert!(state.is_active());

        let r2 = state.apply("foo bar");
        assert_eq!(r2, "foo_bar");
    }

    #[test]
    fn test_mode_state_one_shot() {
        let mut state = SpellingModeState::new();
        state.set_one_shot(SpellingMode::Camel);
        assert!(state.is_active());

        let r1 = state.apply("hello world");
        assert_eq!(r1, "helloWorld");
        // One-shot: should reset after apply
        assert!(!state.is_active());

        let r2 = state.apply("foo bar");
        assert_eq!(r2, "foo bar"); // no transformation
    }

    #[test]
    fn test_nato_digits() {
        assert_eq!(apply_mode("one two three", SpellingMode::Spelling), "123");
    }

    #[test]
    fn test_empty_input() {
        assert_eq!(apply_mode("", SpellingMode::Snake), "");
    }

    #[test]
    fn test_single_word_snake() {
        assert_eq!(apply_mode("hello", SpellingMode::Snake), "hello");
    }
}
