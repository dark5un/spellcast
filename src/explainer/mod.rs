// SPDX-License-Identifier: Apache-2.0

//! Explain feature — concept-to-token via DB → LLM → web search.
//!
//! When the user presses `E` and speaks an explanation, VoxKey:
//! 1. Checks the local SQLite DB for a cached explanation → token mapping
//! 2. Falls back to a local LLM query
//! 3. Falls back to a web search
//! 4. Stores the result in the DB for future use

use std::collections::HashMap;

use sha2::{Digest, Sha256};

use crate::error::{VoxKeyError, VoxKeyResult};
use crate::memory::MemoryStore;

/// Configuration for the explainer.
#[derive(Debug, Clone)]
pub struct ExplainerConfig {
    /// Whether the LLM backend is available
    pub llm_available: bool,
    /// Whether web search is available
    pub web_search_available: bool,
    /// Confidence threshold for cache hits (0.0 - 1.0)
    pub cache_confidence_threshold: f64,
}

impl Default for ExplainerConfig {
    fn default() -> Self {
        Self {
            llm_available: false,
            web_search_available: true,
            cache_confidence_threshold: 0.7,
        }
    }
}

/// Result of an explain operation.
#[derive(Debug, Clone)]
pub struct ExplainResult {
    /// The resulting token/phrase.
    pub token: String,
    /// How the result was obtained.
    pub source: ExplainSource,
    /// Confidence level (0.0 - 1.0).
    pub confidence: f64,
}

/// Source of an explain result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExplainSource {
    /// From the local cache (SQLite DB)
    LocalCache,
    /// From the LLM
    Llm,
    /// From web search
    WebSearch,
}

impl std::fmt::Display for ExplainSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExplainSource::LocalCache => write!(f, "cache"),
            ExplainSource::Llm => write!(f, "LLM"),
            ExplainSource::WebSearch => write!(f, "web"),
        }
    }
}

/// The explainer engine.
pub struct Explainer {
    /// Configuration
    config: ExplainerConfig,
    /// Memory store reference
    memory: Option<MemoryStore>,
}

impl Explainer {
    /// Create a new Explainer.
    pub fn new(config: ExplainerConfig) -> Self {
        Self {
            config,
            memory: None,
        }
    }

    /// Attach a memory store for caching.
    pub fn with_memory(mut self, memory: MemoryStore) -> Self {
        self.memory = Some(memory);
        self
    }

    /// Compute a hash for the explanation text (for DB lookup).
    pub fn hash_explanation(&self, explanation: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(explanation.as_bytes());
        let result = hasher.finalize();
        hex::encode(&result[..8]) // First 8 bytes for shorter keys
    }

    /// Run the explain pipeline.
    ///
    /// 1. Check local cache
    /// 2. Fall back to LLM (if available)
    /// 3. Fall back to web search
    pub fn explain(&self, explanation: &str, context: &str) -> VoxKeyResult<ExplainResult> {
        let explanation = explanation.trim().to_lowercase();

        // Step 1: Check local cache
        if let Some(ref memory) = self.memory {
            let explanation_hash = self.hash_explanation(&explanation);
            if let Some(cached) = memory.lookup_explained(&explanation_hash)? {
                if cached.usage_count > 0 {
                    log::info!("Explain: cache hit for '{}'", explanation);
                    return Ok(ExplainResult {
                        token: cached.token,
                        source: ExplainSource::LocalCache,
                        confidence: 0.9,
                    });
                }
            }
        }

        // Step 2: LLM fallback
        #[cfg(feature = "llm")]
        if self.config.llm_available {
            match self.query_llm(&explanation, context) {
                Ok(result) => {
                    if let Some(ref memory) = self.memory {
                        let _ = memory.store_explanation(
                            context,
                            &explanation,
                            &result.token,
                        );
                    }
                    return Ok(result);
                }
                Err(e) => {
                    log::warn!("LLM explain failed: {e}, falling back to web search");
                }
            }
        }

        // Step 3: Web search fallback
        if self.config.web_search_available {
            match self.web_search(&explanation) {
                Ok(result) => {
                    if let Some(ref memory) = self.memory {
                        let _ = memory.store_explanation(
                            context,
                            &explanation,
                            &result.token,
                        );
                    }
                    return Ok(result);
                }
                Err(e) => {
                    log::warn!("Web search explain failed: {e}");
                }
            }
        }

        // All fallbacks exhausted
        Err(VoxKeyError::ExplainerDb(
            "All explain sources exhausted".to_string(),
        ))
    }

    /// Query a local LLM via web-like API (stub for MVP).
    #[cfg(feature = "llm")]
    fn query_llm(&self, explanation: &str, context: &str) -> VoxKeyResult<ExplainResult> {
        use mistralrs::{IsqBits, ModelBuilder, TextMessageRole, TextMessages};

        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| VoxKeyError::Llm(format!("Failed to create runtime: {e}")))?;

        let result = rt.block_on(async {
            let model = ModelBuilder::new("Qwen/Qwen3-4B")
                .with_auto_isq(IsqBits::Four)
                .build()
                .await
                .map_err(|e| VoxKeyError::Llm(format!("Failed to load model: {e}")))?;

            let prompt = format!(
                "Given the explanation \"{}\" in the context \"{}\", \
                 what is the most likely single word or short phrase being described? \
                 Respond with only the word or phrase, nothing else.",
                explanation, context
            );

            let messages = TextMessages::new()
                .add_message(TextMessageRole::User, &prompt);

            let response = model
                .send_chat_request(messages)
                .await
                .map_err(|e| VoxKeyError::Llm(format!("LLM request failed: {e}")))?;

            let text = response
                .choices
                .first()
                .and_then(|c| c.message.content.as_ref())
                .map(|s| s.trim().to_string())
                .unwrap_or_default();

            Ok::<ExplainResult, VoxKeyError>(ExplainResult {
                token: text,
                source: ExplainSource::Llm,
                confidence: 0.7,
            })
        });

        result
    }

    /// Web search fallback (stub for MVP).
    ///
    /// In the MVP, this searches a hardcoded dictionary or returns an error.
    /// A production version would use DuckDuckGo or a similar API.
    fn web_search(&self, explanation: &str) -> VoxKeyResult<ExplainResult> {
        // Try a simple dictionary lookup first (MVP stub)
        let dictionary = create_dictionary();

        if let Some(token) = search_dictionary(&dictionary, explanation) {
            log::info!("Explain: dictionary hit for '{}'", explanation);
            return Ok(ExplainResult {
                token,
                source: ExplainSource::WebSearch,
                confidence: 0.5,
            });
        }

        // For MVP: if no dictionary match, return the explanation itself
        // as a best-effort fallback
        if !explanation.is_empty() {
            log::info!("Explain: returning raw explanation as token");
            return Ok(ExplainResult {
                token: explanation.to_string(),
                source: ExplainSource::WebSearch,
                confidence: 0.3,
            });
        }

        Err(VoxKeyError::WebSearch("No results found".to_string()))
    }
}

/// A simple concept → token dictionary for the MVP.
type Dictionary = Vec<(String, String)>;

fn create_dictionary() -> Dictionary {
    vec![
        ("iterate over a collection".to_string(), "for loop".to_string()),
        ("conditional execution".to_string(), "if statement".to_string()),
        ("function that returns nothing".to_string(), "void".to_string()),
        ("a sequence of characters".to_string(), "string".to_string()),
        ("a whole number".to_string(), "integer".to_string()),
        ("a decimal number".to_string(), "float".to_string()),
        ("true or false value".to_string(), "boolean".to_string()),
        ("a collection of items".to_string(), "array".to_string()),
        ("a key value store".to_string(), "hash map".to_string()),
        ("a named block of code".to_string(), "function".to_string()),
        ("a value that doesn't change".to_string(), "constant".to_string()),
        ("a named memory location".to_string(), "variable".to_string()),
        ("the current date".to_string(), "today".to_string()),
        ("the person reading this".to_string(), "you".to_string()),
        ("a greeting".to_string(), "hello".to_string()),
        ("leave taking".to_string(), "goodbye".to_string()),
        ("to make something".to_string(), "create".to_string()),
        ("to find a bug".to_string(), "debug".to_string()),
        ("to put into action".to_string(), "execute".to_string()),
        ("to bring together".to_string(), "merge".to_string()),
    ]
}

fn search_dictionary(dict: &Dictionary, query: &str) -> Option<String> {
    let query_lower = query.to_lowercase();

    // Exact match on the concept description
    for (concept, token) in dict {
        if concept.to_lowercase() == query_lower {
            return Some(token.clone());
        }
    }

    // Partial match
    for (concept, token) in dict {
        if concept.to_lowercase().contains(&query_lower)
            || query_lower.contains(&concept.to_lowercase())
        {
            return Some(token.clone());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_explainer() -> Explainer {
        let config = ExplainerConfig {
            llm_available: false,
            web_search_available: true,
            cache_confidence_threshold: 0.7,
        };
        Explainer::new(config)
    }

    #[test]
    fn test_explain_dictionary_hit() {
        let e = setup_explainer();
        let result = e.explain("a collection of items", "prose").unwrap();
        assert_eq!(result.token, "array");
        assert_eq!(result.source, ExplainSource::WebSearch);
    }

    #[test]
    fn test_explain_dictionary_partial_match() {
        let e = setup_explainer();
        let result = e.explain("collection of items", "prose").unwrap();
        assert_eq!(result.token, "array");
    }

    #[test]
    fn test_explain_fallback_to_raw_text() {
        let e = setup_explainer();
        let result = e.explain("something completely unique", "prose").unwrap();
        assert_eq!(result.token, "something completely unique");
    }

    #[test]
    fn test_hash_explanation() {
        let e = setup_explainer();
        let hash1 = e.hash_explanation("a collection of items");
        let hash2 = e.hash_explanation("a collection of items");
        let hash3 = e.hash_explanation("different thing");
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert_eq!(hash1.len(), 16); // 8 bytes = 16 hex chars
    }

    #[test]
    fn test_explain_source_display() {
        assert_eq!(ExplainSource::LocalCache.to_string(), "cache");
        assert_eq!(ExplainSource::Llm.to_string(), "LLM");
        assert_eq!(ExplainSource::WebSearch.to_string(), "web");
    }

    #[test]
    fn test_explain_empty_explanation() {
        let e = setup_explainer();
        let result = e.explain("", "prose");
        // Empty explanation should still return something
        assert!(result.is_ok());
    }

    #[test]
    fn test_dictionary_search() {
        let dict = create_dictionary();
        let result = search_dictionary(&dict, "a greeting");
        assert_eq!(result, Some("hello".to_string()));

        let result = search_dictionary(&dict, "nonexistent thing");
        assert_eq!(result, None);
    }
}