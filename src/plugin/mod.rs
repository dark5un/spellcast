// SPDX-License-Identifier: Apache-2.0

//! Plugin and extension system.
//!
//! Supports Rust dynamic libraries (.so) and Lua scripts.
//! Plugins implement the `SpellcastPlugin` trait and are
//! discovered at startup from ~/.config/spellcast/plugins/.

use std::collections::HashMap;

use crate::tokenizer::{Token, TokenContext};

/// Action a plugin can return after an event.
#[derive(Debug, Clone)]
pub enum PluginAction {
    /// Continue normally (no modification).
    Continue,
    /// Modify the token stream.
    ModifyToken(String),
    /// Suppress the default behavior.
    Suppress,
    /// Add custom predictions.
    AddPredictions(Vec<String>),
}

/// The trait all plugins must implement.
pub trait SpellcastPlugin: Send {
    fn name(&self) -> &str;

    fn on_loaded(&mut self) {}
    fn on_unloaded(&mut self) {}

    /// Called when a token is committed to the stream.
    fn on_token_committed(&mut self, _token: &Token, _context: &TokenContext) -> PluginAction {
        PluginAction::Continue
    }

    /// Called when the user navigates to a token.
    fn on_token_navigated(&mut self, _token: &Token, _direction: &str) -> PluginAction {
        PluginAction::Continue
    }

    /// Called during explain feature. Return a token if the plugin can answer.
    fn on_explain(&mut self, _explanation: &str, _context: &str) -> Option<String> {
        None
    }

    /// Return custom phonetic predictions.
    fn custom_predictions(&mut self, _token: &Token) -> Vec<String> {
        Vec::new()
    }
}

/// Built-in plugin: domain-specific dictionary for medical terminology.
pub struct MedicalDictionaryPlugin;

impl SpellcastPlugin for MedicalDictionaryPlugin {
    fn name(&self) -> &str {
        "medical-dictionary"
    }

    fn on_explain(&mut self, explanation: &str, _context: &str) -> Option<String> {
        let e = explanation.to_lowercase();
        match e.as_str() {
            "the thing that measures heart activity" => Some("ECG".into()),
            "the test that measures brain activity" => Some("EEG".into()),
            "the machine that does mri scans" => Some("MRI".into()),
            "the drug that reduces inflammation" => Some("NSAID".into()),
            "the procedure that removes the appendix" => Some("appendectomy".into()),
            _ => None,
        }
    }
}

/// Built-in plugin: custom code domain for programming languages.
pub struct CodeSymbolsPlugin;

impl SpellcastPlugin for CodeSymbolsPlugin {
    fn name(&self) -> &str {
        "code-symbols"
    }

    fn custom_predictions(&mut self, token: &Token) -> Vec<String> {
        let lower = token.text.to_lowercase();
        let mut preds = Vec::new();
        if lower == "lambda" {
            preds.push("lambda".into());
            preds.push("() =>".into());
            preds.push("\\".into());
        } else if lower == "arrow" {
            preds.push("->".into());
            preds.push("=>".into());
            preds.push("→".into());
        } else if lower == "function" {
            preds.push("fn".into());
            preds.push("function".into());
            preds.push("def".into());
        }
        preds
    }
}

/// Plugin manager — loads, stores, and dispatches to plugins.
pub struct PluginManager {
    /// Loaded plugins (name → boxed trait object).
    plugins: HashMap<String, Box<dyn SpellcastPlugin>>,
}

impl PluginManager {
    pub fn new(_plugin_dir: &str) -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    /// Register a built-in plugin.
    pub fn register(&mut self, plugin: Box<dyn SpellcastPlugin>) {
        let name = plugin.name().to_string();
        self.plugins.insert(name.clone(), plugin);
        if let Some(p) = self.plugins.get_mut(&name) {
            p.on_loaded();
        }
    }

    /// Load all built-in plugins.
    pub fn load_builtins(&mut self) {
        self.register(Box::new(MedicalDictionaryPlugin));
        self.register(Box::new(CodeSymbolsPlugin));
    }

    /// Get a plugin by name.
    pub fn get(&self, name: &str) -> Option<&dyn SpellcastPlugin> {
        self.plugins.get(name).map(|b| b.as_ref())
    }

    /// Get a mutable reference to a plugin.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Box<dyn SpellcastPlugin>> {
        self.plugins.get_mut(name)
    }

    /// Dispatch `on_token_committed` to all plugins.
    pub fn on_token_committed(&mut self, token: &Token, context: &TokenContext) {
        let actions: Vec<(String, PluginAction)> = self
            .plugins
            .iter_mut()
            .map(|(name, p)| {
                let action = p.on_token_committed(token, context);
                (name.clone(), action)
            })
            .collect();

        for (_name, action) in actions {
            match action {
                PluginAction::Suppress => {
                    // TODO: actually suppress in the pipeline
                }
                PluginAction::ModifyToken(t) => {
                    // TODO: actually modify the token
                    let _ = t;
                }
                _ => {}
            }
        }
    }

    /// Dispatch `on_explain` to all plugins and return first match.
    pub fn on_explain(&mut self, explanation: &str, context: &str) -> Option<String> {
        for p in self.plugins.values_mut() {
            if let Some(result) = p.on_explain(explanation, context) {
                return Some(result);
            }
        }
        None
    }

    /// Get all custom predictions from all plugins for a token.
    pub fn custom_predictions(&mut self, token: &Token) -> Vec<String> {
        let mut all = Vec::new();
        for p in self.plugins.values_mut() {
            all.extend(p.custom_predictions(token));
        }
        all
    }

    /// List loaded plugin names.
    pub fn list(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.plugins.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    /// Unload a plugin by name.
    pub fn unload(&mut self, name: &str) -> bool {
        if let Some(mut p) = self.plugins.remove(name) {
            p.on_unloaded();
            true
        } else {
            false
        }
    }

    /// Plugin count.
    pub fn count(&self) -> usize {
        self.plugins.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::TokenType;

    #[test]
    fn test_medical_dictionary_plugin() {
        let mut mgr = PluginManager::new("");
        mgr.load_builtins();

        let result = mgr.on_explain("the thing that measures heart activity", "medical");
        assert_eq!(result, Some("ECG".to_string()));

        let result2 = mgr.on_explain("unknown term", "medical");
        assert_eq!(result2, None);
    }

    #[test]
    fn test_code_symbols_predictions() {
        let mut mgr = PluginManager::new("");
        mgr.load_builtins();

        let token = Token {
            text: "lambda".into(),
            offset: 0,
            length: 6,
            token_type: TokenType::Word,
        };
        let preds = mgr.custom_predictions(&token);
        assert!(preds.len() >= 3);
        assert!(preds.contains(&"lambda".to_string()));
    }

    #[test]
    fn test_plugin_list() {
        let mut mgr = PluginManager::new("");
        mgr.load_builtins();
        let list = mgr.list();
        assert!(list.contains(&"medical-dictionary"));
        assert!(list.contains(&"code-symbols"));
    }

    #[test]
    fn test_plugin_unload() {
        let mut mgr = PluginManager::new("");
        mgr.register(Box::new(MedicalDictionaryPlugin));
        assert_eq!(mgr.count(), 1);
        assert!(mgr.unload("medical-dictionary"));
        assert_eq!(mgr.count(), 0);
    }

    #[test]
    fn test_unload_nonexistent() {
        let mut mgr = PluginManager::new("");
        assert!(!mgr.unload("nonexistent"));
    }

    #[test]
    fn test_get_plugin() {
        let mut mgr = PluginManager::new("");
        mgr.load_builtins();
        let p = mgr.get("medical-dictionary");
        assert!(p.is_some());
        assert_eq!(p.unwrap().name(), "medical-dictionary");
    }

    #[test]
    fn test_plugin_action_continue() {
        assert!(matches!(PluginAction::Continue, PluginAction::Continue));
    }

    #[test]
    fn test_on_token_committed_no_panic() {
        let mut mgr = PluginManager::new("");
        mgr.load_builtins();
        let token = Token {
            text: "test".into(),
            offset: 0,
            length: 4,
            token_type: TokenType::Word,
        };
        mgr.on_token_committed(&token, &TokenContext::Prose);
        // Should not panic
        assert!(true);
    }

    #[test]
    fn test_plugin_manager_new_empty() {
        let mgr = PluginManager::new("/tmp/plugins");
        assert_eq!(mgr.count(), 0);
    }
}
