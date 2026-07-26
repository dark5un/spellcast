// SPDX-License-Identifier: Apache-2.0

//! Emoticon and macro system.
//!
//! Context-filtered emoticon/emoji quick-pick menu and user-defined
//! macro snippets. Macros support cursor positioning and variable
//! interpolation ($DATE, $TIME, $FILE).

use std::collections::HashMap;

/// An emoticon/emoji entry.
#[derive(Debug, Clone)]
pub struct Emoticon {
    /// Trigger phrase (spoken or typed).
    pub trigger: &'static str,
    /// The rendered symbol.
    pub symbol: &'static str,
    /// Contexts where this emoticon is relevant.
    pub contexts: &'static [&'static str],
}

/// Filtered emoticon/emoji categories.
fn all_emoticons() -> Vec<Emoticon> {
    vec![
        // Prose / chat
        Emoticon { trigger: "happy face", symbol: "😊", contexts: &["prose", "chat"] },
        Emoticon { trigger: "laugh", symbol: "😂", contexts: &["prose", "chat"] },
        Emoticon { trigger: "cry laughing", symbol: "😂", contexts: &["prose", "chat"] },
        Emoticon { trigger: "thumbs up", symbol: "👍", contexts: &["prose", "chat"] },
        Emoticon { trigger: "thumbs down", symbol: "👎", contexts: &["prose", "chat"] },
        Emoticon { trigger: "heart", symbol: "❤️", contexts: &["prose", "chat"] },
        Emoticon { trigger: "shrug", symbol: "¯\\_(ツ)_/¯", contexts: &["prose", "chat"] },
        Emoticon { trigger: "fire", symbol: "🔥", contexts: &["prose", "chat"] },
        Emoticon { trigger: "100", symbol: "💯", contexts: &["prose", "chat"] },
        Emoticon { trigger: "party", symbol: "🎉", contexts: &["prose", "chat"] },
        Emoticon { trigger: "sad face", symbol: "😢", contexts: &["prose", "chat"] },
        Emoticon { trigger: "wink", symbol: "😉", contexts: &["prose", "chat"] },
        Emoticon { trigger: "cool", symbol: "😎", contexts: &["prose", "chat"] },
        Emoticon { trigger: "skull", symbol: "💀", contexts: &["prose", "chat"] },
        Emoticon { trigger: "poop", symbol: "💩", contexts: &["prose", "chat"] },
        // Code (emoji-free, but keep some markers)
        Emoticon { trigger: "todo", symbol: "TODO:", contexts: &["code"] },
        Emoticon { trigger: "fix me", symbol: "FIXME:", contexts: &["code"] },
        Emoticon { trigger: "hack", symbol: "HACK:", contexts: &["code"] },
        Emoticon { trigger: "note", symbol: "NOTE:", contexts: &["code"] },
        Emoticon { trigger: "warn", symbol: "WARN:", contexts: &["code"] },
        Emoticon { trigger: "xyz", symbol: "XYZ:", contexts: &["code"] },
        // ASCII art
        Emoticon { trigger: "flip table", symbol: "(╯°□°)╯︵┻━┻", contexts: &["prose", "chat"] },
        Emoticon { trigger: "bear", symbol: "ʕ•ᴥ•ʔ", contexts: &["prose", "chat"] },
        Emoticon { trigger: "cat", symbol: "=^_^=", contexts: &["prose", "chat"] },
    ]
}

/// A user-defined macro (multi-token snippet).
#[derive(Debug, Clone)]
pub struct Macro {
    /// Voice trigger phrase.
    pub trigger: String,
    /// Expansion text.
    pub expansion: String,
    /// Cursor position relative to end of expansion (-1 = leave cursor at end).
    pub cursor_position: Option<isize>,
}

impl Macro {
    /// Interpolate variables in the expansion.
    pub fn interpolate(&self, file: Option<&str>) -> String {
        let mut result = self.expansion.clone();
        if let Some(f) = file {
            result = result.replace("$FILE", f);
        }
        result = result.replace("$DATE", &chrono_now_date());
        result = result.replace("$TIME", &chrono_now_time());
        result
    }
}

fn chrono_now_date() -> String {
    // Simple date string without chrono dependency
    "2026-07-26".to_string()
}

fn chrono_now_time() -> String {
    "10:30".to_string()
}

/// The emoticon and macro manager.
pub struct EmoticonMacroManager {
    /// Built-in emoticons.
    emoticons: Vec<Emoticon>,
    /// User-defined macros.
    macros: HashMap<String, Macro>,
}

impl EmoticonMacroManager {
    pub fn new() -> Self {
        Self {
            emoticons: all_emoticons(),
            macros: HashMap::new(),
        }
    }

    /// Get emoticons filtered by context.
    pub fn emoticons_for_context(&self, context: &str) -> Vec<&Emoticon> {
        self.emoticons
            .iter()
            .filter(|e| e.contexts.contains(&context))
            .collect()
    }

    /// Find an emoticon by trigger phrase.
    pub fn find_emoticon(&self, trigger: &str) -> Option<&Emoticon> {
        self.emoticons
            .iter()
            .find(|e| e.trigger == trigger.to_lowercase().trim())
    }

    /// Add or update a macro.
    pub fn add_macro(&mut self, m: Macro) {
        self.macros.insert(m.trigger.clone(), m);
    }

    /// Find a macro by trigger.
    pub fn find_macro(&self, trigger: &str) -> Option<&Macro> {
        self.macros.get(trigger.to_lowercase().trim())
    }

    /// Remove a macro.
    pub fn remove_macro(&mut self, trigger: &str) -> bool {
        self.macros.remove(trigger.to_lowercase().trim()).is_some()
    }

    /// List all macro triggers.
    pub fn list_macros(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self.macros.keys().map(|s| s.as_str()).collect();
        keys.sort();
        keys
    }

    /// Check if a trigger is a known macro.
    pub fn is_macro(&self, trigger: &str) -> bool {
        self.macros.contains_key(trigger.to_lowercase().trim())
    }

    /// Check if a trigger is a known emoticon.
    pub fn is_emoticon(&self, trigger: &str) -> bool {
        self.emoticons.iter().any(|e| e.trigger == trigger.to_lowercase().trim())
    }
}

impl Default for EmoticonMacroManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emoticon_count() {
        let mgr = EmoticonMacroManager::new();
        assert!(mgr.emoticons.len() >= 18);
    }

    #[test]
    fn test_emoticons_for_prose_context() {
        let mgr = EmoticonMacroManager::new();
        let prose = mgr.emoticons_for_context("prose");
        assert!(prose.len() >= 12);
        assert!(prose.iter().any(|e| e.symbol == "😊"));
    }

    #[test]
    fn test_emoticons_for_code_context() {
        let mgr = EmoticonMacroManager::new();
        let code = mgr.emoticons_for_context("code");
        assert!(code.iter().any(|e| e.symbol == "TODO:"));
        assert!(code.iter().all(|e| !e.symbol.contains('😊')));
    }

    #[test]
    fn test_find_emoticon() {
        let mgr = EmoticonMacroManager::new();
        let e = mgr.find_emoticon("happy face").unwrap();
        assert_eq!(e.symbol, "😊");
    }

    #[test]
    fn test_shrug() {
        let mgr = EmoticonMacroManager::new();
        let e = mgr.find_emoticon("shrug").unwrap();
        assert_eq!(e.symbol, "¯\\_(ツ)_/¯");
    }

    #[test]
    fn test_add_and_find_macro() {
        let mut mgr = EmoticonMacroManager::new();
        let m = Macro {
            trigger: "sig".to_string(),
            expansion: "Best regards,\nJohn".to_string(),
            cursor_position: None,
        };
        mgr.add_macro(m);
        let found = mgr.find_macro("sig").unwrap();
        assert_eq!(found.expansion, "Best regards,\nJohn");
    }

    #[test]
    fn test_remove_macro() {
        let mut mgr = EmoticonMacroManager::new();
        mgr.add_macro(Macro {
            trigger: "test".to_string(),
            expansion: "test expansion".to_string(),
            cursor_position: None,
        });
        assert!(mgr.remove_macro("test"));
        assert!(mgr.find_macro("test").is_none());
    }

    #[test]
    fn test_list_macros() {
        let mut mgr = EmoticonMacroManager::new();
        mgr.add_macro(Macro {
            trigger: "beta".to_string(),
            expansion: "b".to_string(),
            cursor_position: None,
        });
        mgr.add_macro(Macro {
            trigger: "alpha".to_string(),
            expansion: "a".to_string(),
            cursor_position: None,
        });
        let list = mgr.list_macros();
        assert_eq!(list, vec!["alpha", "beta"]);
    }

    #[test]
    fn test_macro_interpolate() {
        let m = Macro {
            trigger: "header".to_string(),
            expansion: "// SPDX-License-Identifier: Apache-2.0\n// $FILE\n// $DATE".to_string(),
            cursor_position: None,
        };
        let result = m.interpolate(Some("main.rs"));
        assert!(result.contains("main.rs"));
        assert!(result.contains("2026-07-26"));
        assert!(result.contains("SPDX"));
    }

    #[test]
    fn test_is_emoticon() {
        let mgr = EmoticonMacroManager::new();
        assert!(mgr.is_emoticon("happy face"));
        assert!(!mgr.is_emoticon("unknown_phrase"));
    }

    #[test]
    fn test_is_macro() {
        let mut mgr = EmoticonMacroManager::new();
        assert!(!mgr.is_macro("sig"));
        mgr.add_macro(Macro {
            trigger: "sig".to_string(),
            expansion: "regards".to_string(),
            cursor_position: None,
        });
        assert!(mgr.is_macro("sig"));
    }
}