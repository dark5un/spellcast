// SPDX-License-Identifier: Apache-2.0

//! Symbol dictation — verbal commands for symbols and operators.
//!
//! Provides a mapping from spoken phrases to symbol characters,
//! with context-dependent resolution (e.g., "bracket" means []
//! in most code but <> in HTML/XML).

use std::collections::HashMap;

/// A symbol entry with context-dependent alternatives.
#[derive(Debug, Clone)]
pub struct SymbolEntry {
    /// Default symbol (used for most contexts).
    pub default: &'static str,
    /// Context-dependent overrides.
    pub overrides: Vec<(&'static str, &'static str)>,
}

impl SymbolEntry {
    /// Resolve the symbol for a given context.
    pub fn resolve(&self, context: &str) -> &'static str {
        for (lang, symbol) in &self.overrides {
            if *lang == context {
                return symbol;
            }
        }
        self.default
    }
}

/// The symbol dictionary — maps spoken phrases to their symbol equivalents.
pub struct SymbolDictionary {
    /// The core symbol map.
    entries: HashMap<String, SymbolEntry>,
}

impl SymbolDictionary {
    /// Create the default symbol dictionary.
    pub fn new() -> Self {
        let mut entries = HashMap::new();

        // Parentheses
        entries.insert(
            "open paren".to_string(),
            SymbolEntry {
                default: "(",
                overrides: vec![],
            },
        );
        entries.insert(
            "close paren".to_string(),
            SymbolEntry {
                default: ")",
                overrides: vec![],
            },
        );
        entries.insert(
            "left paren".to_string(),
            SymbolEntry {
                default: "(",
                overrides: vec![],
            },
        );
        entries.insert(
            "right paren".to_string(),
            SymbolEntry {
                default: ")",
                overrides: vec![],
            },
        );

        // Brackets
        entries.insert(
            "open bracket".to_string(),
            SymbolEntry {
                default: "[",
                overrides: vec![("html", "&lt;"), ("xml", "&lt;")],
            },
        );
        entries.insert(
            "close bracket".to_string(),
            SymbolEntry {
                default: "]",
                overrides: vec![("html", "&gt;"), ("xml", "&gt;")],
            },
        );
        entries.insert(
            "left bracket".to_string(),
            SymbolEntry {
                default: "[",
                overrides: vec![("html", "&lt;"), ("xml", "&lt;")],
            },
        );
        entries.insert(
            "right bracket".to_string(),
            SymbolEntry {
                default: "]",
                overrides: vec![("html", "&gt;"), ("xml", "&gt;")],
            },
        );

        // Braces
        entries.insert(
            "open brace".to_string(),
            SymbolEntry {
                default: "{",
                overrides: vec![],
            },
        );
        entries.insert(
            "close brace".to_string(),
            SymbolEntry {
                default: "}",
                overrides: vec![],
            },
        );
        entries.insert(
            "left brace".to_string(),
            SymbolEntry {
                default: "{",
                overrides: vec![],
            },
        );
        entries.insert(
            "right brace".to_string(),
            SymbolEntry {
                default: "}",
                overrides: vec![],
            },
        );

        // Operators
        entries.insert(
            "equals".to_string(),
            SymbolEntry {
                default: "=",
                overrides: vec![],
            },
        );
        entries.insert(
            "double equals".to_string(),
            SymbolEntry {
                default: "==",
                overrides: vec![],
            },
        );
        entries.insert(
            "arrow".to_string(),
            SymbolEntry {
                default: "->",
                overrides: vec![("javascript", "=>"), ("typescript", "=>"), ("rust", "=>")],
            },
        );
        entries.insert(
            "pipe".to_string(),
            SymbolEntry {
                default: "|",
                overrides: vec![],
            },
        );
        entries.insert(
            "double pipe".to_string(),
            SymbolEntry {
                default: "||",
                overrides: vec![],
            },
        );
        entries.insert(
            "ampersand".to_string(),
            SymbolEntry {
                default: "&",
                overrides: vec![],
            },
        );
        entries.insert(
            "double ampersand".to_string(),
            SymbolEntry {
                default: "&&",
                overrides: vec![],
            },
        );
        entries.insert(
            "bang".to_string(),
            SymbolEntry {
                default: "!",
                overrides: vec![],
            },
        );
        entries.insert(
            "bang equals".to_string(),
            SymbolEntry {
                default: "!=",
                overrides: vec![],
            },
        );

        // Punctuation
        entries.insert(
            "semicolon".to_string(),
            SymbolEntry {
                default: ";",
                overrides: vec![],
            },
        );
        entries.insert(
            "colon".to_string(),
            SymbolEntry {
                default: ":",
                overrides: vec![],
            },
        );
        entries.insert(
            "colon colon".to_string(),
            SymbolEntry {
                default: "::",
                overrides: vec![],
            },
        );
        entries.insert(
            "question mark".to_string(),
            SymbolEntry {
                default: "?",
                overrides: vec![],
            },
        );
        entries.insert(
            "dot".to_string(),
            SymbolEntry {
                default: ".",
                overrides: vec![],
            },
        );
        entries.insert(
            "period".to_string(),
            SymbolEntry {
                default: ".",
                overrides: vec![],
            },
        );

        // Slashes
        entries.insert(
            "slash".to_string(),
            SymbolEntry {
                default: "/",
                overrides: vec![],
            },
        );
        entries.insert(
            "forward slash".to_string(),
            SymbolEntry {
                default: "/",
                overrides: vec![],
            },
        );
        entries.insert(
            "backslash".to_string(),
            SymbolEntry {
                default: "\\",
                overrides: vec![],
            },
        );

        // Other
        entries.insert(
            "star".to_string(),
            SymbolEntry {
                default: "*",
                overrides: vec![],
            },
        );
        entries.insert(
            "asterisk".to_string(),
            SymbolEntry {
                default: "*",
                overrides: vec![],
            },
        );
        entries.insert(
            "hash".to_string(),
            SymbolEntry {
                default: "#",
                overrides: vec![],
            },
        );
        entries.insert(
            "pound".to_string(),
            SymbolEntry {
                default: "#",
                overrides: vec![],
            },
        );
        entries.insert(
            "at sign".to_string(),
            SymbolEntry {
                default: "@",
                overrides: vec![],
            },
        );
        entries.insert(
            "tilde".to_string(),
            SymbolEntry {
                default: "~",
                overrides: vec![],
            },
        );
        entries.insert(
            "backtick".to_string(),
            SymbolEntry {
                default: "`",
                overrides: vec![],
            },
        );
        entries.insert(
            "dollar".to_string(),
            SymbolEntry {
                default: "$",
                overrides: vec![],
            },
        );
        entries.insert(
            "percent".to_string(),
            SymbolEntry {
                default: "%",
                overrides: vec![],
            },
        );
        entries.insert(
            "caret".to_string(),
            SymbolEntry {
                default: "^",
                overrides: vec![],
            },
        );
        entries.insert(
            "underscore".to_string(),
            SymbolEntry {
                default: "_",
                overrides: vec![],
            },
        );
        entries.insert(
            "plus".to_string(),
            SymbolEntry {
                default: "+",
                overrides: vec![],
            },
        );
        entries.insert(
            "minus".to_string(),
            SymbolEntry {
                default: "-",
                overrides: vec![],
            },
        );
        entries.insert(
            "less than".to_string(),
            SymbolEntry {
                default: "<",
                overrides: vec![("html", "&lt;"), ("xml", "&lt;")],
            },
        );
        entries.insert(
            "greater than".to_string(),
            SymbolEntry {
                default: ">",
                overrides: vec![("html", "&gt;"), ("xml", "&gt;")],
            },
        );

        // Code-specific compound symbols
        entries.insert(
            "dot dot".to_string(),
            SymbolEntry {
                default: "..",
                overrides: vec![("rust", "..="), ("javascript", "...")],
            },
        );
        entries.insert(
            "dot dot dot".to_string(),
            SymbolEntry {
                default: "...",
                overrides: vec![("rust", "..=")],
            },
        );

        Self { entries }
    }

    /// Look up a spoken phrase and return its symbol, resolved for the given context.
    pub fn lookup(&self, phrase: &str, context: &str) -> Option<&'static str> {
        let phrase = phrase.trim().to_lowercase();
        self.entries
            .get(&phrase)
            .map(|entry| entry.resolve(context))
    }

    /// Check if a phrase is a known symbol command.
    pub fn is_symbol_command(&self, phrase: &str) -> bool {
        let phrase = phrase.trim().to_lowercase();
        self.entries.contains_key(&phrase)
    }

    /// Add or override a symbol entry at runtime.
    pub fn add_entry(&mut self, phrase: String, entry: SymbolEntry) {
        self.entries.insert(phrase, entry);
    }

    /// Get all known symbol phrases.
    pub fn known_phrases(&self) -> Vec<&str> {
        let mut phrases: Vec<&str> = self.entries.keys().map(|s| s.as_str()).collect();
        phrases.sort();
        phrases
    }
}

impl Default for SymbolDictionary {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_paren() {
        let dict = SymbolDictionary::new();
        assert_eq!(dict.lookup("open paren", "rust"), Some("("));
    }

    #[test]
    fn test_arrow_javascript() {
        let dict = SymbolDictionary::new();
        assert_eq!(dict.lookup("arrow", "javascript"), Some("=>"));
    }

    #[test]
    fn test_arrow_rust() {
        let dict = SymbolDictionary::new();
        assert_eq!(dict.lookup("arrow", "rust"), Some("=>"));
    }

    #[test]
    fn test_arrow_c() {
        let dict = SymbolDictionary::new();
        assert_eq!(dict.lookup("arrow", "c"), Some("->"));
    }

    #[test]
    fn test_arrow_default() {
        let dict = SymbolDictionary::new();
        assert_eq!(dict.lookup("arrow", "prose"), Some("->"));
    }

    #[test]
    fn test_bracket_html() {
        let dict = SymbolDictionary::new();
        assert_eq!(dict.lookup("open bracket", "html"), Some("&lt;"));
        assert_eq!(dict.lookup("close bracket", "html"), Some("&gt;"));
    }

    #[test]
    fn test_bracket_default() {
        let dict = SymbolDictionary::new();
        assert_eq!(dict.lookup("open bracket", "rust"), Some("["));
    }

    #[test]
    fn test_colon_colon() {
        let dict = SymbolDictionary::new();
        assert_eq!(dict.lookup("colon colon", "rust"), Some("::"));
    }

    #[test]
    fn test_dot_dot_rust() {
        let dict = SymbolDictionary::new();
        assert_eq!(dict.lookup("dot dot", "rust"), Some("..="));
    }

    #[test]
    fn test_dot_dot_javascript() {
        let dict = SymbolDictionary::new();
        assert_eq!(dict.lookup("dot dot", "javascript"), Some("..."));
    }

    #[test]
    fn test_dot_dot_default() {
        let dict = SymbolDictionary::new();
        assert_eq!(dict.lookup("dot dot", "python"), Some(".."));
    }

    #[test]
    fn test_unknown_phrase() {
        let dict = SymbolDictionary::new();
        assert_eq!(dict.lookup("floogle", "rust"), None);
    }

    #[test]
    fn test_is_symbol_command() {
        let dict = SymbolDictionary::new();
        assert!(dict.is_symbol_command("open paren"));
        assert!(dict.is_symbol_command("double equals"));
        assert!(!dict.is_symbol_command("hello world"));
    }

    #[test]
    fn test_case_insensitive() {
        let dict = SymbolDictionary::new();
        assert_eq!(dict.lookup("OPEN PAREN", "rust"), Some("("));
        assert_eq!(dict.lookup("Arrow", "rust"), Some("=>"));
    }

    #[test]
    fn test_known_phrases_not_empty() {
        let dict = SymbolDictionary::new();
        let phrases = dict.known_phrases();
        assert!(phrases.len() > 30);
        assert!(phrases.contains(&"arrow"));
        assert!(phrases.contains(&"bang"));
    }
}
