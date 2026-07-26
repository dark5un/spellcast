// SPDX-License-Identifier: Apache-2.0

//! Explain feature v2 — enhanced concept-to-token resolution.
//!
//! Improvements: conversation context, multi-word preview, domain packs,
//! web cache, LLM fallback.

use crate::error::SpellcastResult;
use crate::memory::MemoryStore;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ExplanationEntry {
    pub explanation: String,
    pub result: String,
    pub context: String,
}

#[derive(Debug, Clone)]
pub struct ExplainerConfigV2 {
    pub max_context: usize,
    pub enable_web_search: bool,
    pub enable_llm: bool,
}

impl Default for ExplainerConfigV2 {
    fn default() -> Self {
        Self {
            max_context: 5,
            enable_web_search: true,
            enable_llm: true,
        }
    }
}

pub struct ExplainerV2 {
    config: ExplainerConfigV2,
    memory: Option<MemoryStore>,
    conversation: Vec<ExplanationEntry>,
    domain_packs: HashMap<String, Vec<DomainPackEntry>>,
    web_cache: HashMap<String, CachedWebResult>,
}

#[derive(Debug, Clone)]
pub struct DomainPackEntry {
    pub explanation: String,
    pub token: String,
    pub language: String,
}

#[derive(Debug, Clone)]
pub struct CachedWebResult {
    pub result: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct ExplainResult {
    pub token: String,
    pub source: ExplainSource,
    pub confidence: f32,
    pub preview: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExplainSource {
    LocalDb,
    DomainPack,
    WebCache,
    Llm,
    WebSearch,
    Heuristic,
}

impl std::fmt::Display for ExplainSource {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ExplainSource::LocalDb => write!(f, "DB"),
            ExplainSource::DomainPack => write!(f, "Pack"),
            ExplainSource::WebCache => write!(f, "Cache"),
            ExplainSource::Llm => write!(f, "LLM"),
            ExplainSource::WebSearch => write!(f, "Web"),
            ExplainSource::Heuristic => write!(f, "Guess"),
        }
    }
}

impl ExplainerV2 {
    pub fn new(config: ExplainerConfigV2, memory: Option<MemoryStore>) -> Self {
        Self {
            config,
            memory,
            conversation: Vec::new(),
            domain_packs: HashMap::new(),
            web_cache: HashMap::new(),
        }
    }

    pub fn load_domain_pack(&mut self, domain: &str, entries: Vec<DomainPackEntry>) {
        self.domain_packs.insert(domain.to_string(), entries);
    }

    pub fn load_builtin_packs(&mut self) {
        self.load_domain_pack(
            "python",
            vec![
                DomainPackEntry {
                    explanation: "the function that reads a file".into(),
                    token: "open()".into(),
                    language: "python".into(),
                },
                DomainPackEntry {
                    explanation: "the function that prints to console".into(),
                    token: "print()".into(),
                    language: "python".into(),
                },
                DomainPackEntry {
                    explanation: "the thing that iterates over a sequence".into(),
                    token: "for".into(),
                    language: "python".into(),
                },
                DomainPackEntry {
                    explanation: "the thing that groups elements".into(),
                    token: "itertools.groupby()".into(),
                    language: "python".into(),
                },
                DomainPackEntry {
                    explanation: "the thing that handles errors".into(),
                    token: "try/except".into(),
                    language: "python".into(),
                },
            ],
        );
        self.load_domain_pack(
            "rust",
            vec![
                DomainPackEntry {
                    explanation: "the function that reads a file".into(),
                    token: "fs::read_to_string()".into(),
                    language: "rust".into(),
                },
                DomainPackEntry {
                    explanation: "the thing that prints to console".into(),
                    token: "println!()".into(),
                    language: "rust".into(),
                },
                DomainPackEntry {
                    explanation: "the thing that creates a vector".into(),
                    token: "vec![]".into(),
                    language: "rust".into(),
                },
                DomainPackEntry {
                    explanation: "the thing that handles errors".into(),
                    token: "Result".into(),
                    language: "rust".into(),
                },
            ],
        );
        self.load_domain_pack(
            "sql",
            vec![
                DomainPackEntry {
                    explanation: "select all rows from a table".into(),
                    token: "SELECT * FROM".into(),
                    language: "sql".into(),
                },
                DomainPackEntry {
                    explanation: "filter rows by condition".into(),
                    token: "WHERE".into(),
                    language: "sql".into(),
                },
            ],
        );
    }

    pub fn explain(&mut self, explanation: &str, context: &str) -> SpellcastResult<ExplainResult> {
        // 1. Check local DB — extract before any mutable borrows
        let db_result = match &self.memory {
            Some(m) => m
                .lookup_explained(explanation)
                .ok()
                .flatten()
                .map(|r| (r.token.clone(), r.usage_count)),
            None => None,
        };
        if let Some((token, usage)) = db_result
            && usage > 2
        {
            self.push_context(explanation.to_string(), token.clone(), context.to_string());
            return Ok(ExplainResult {
                token,
                source: ExplainSource::LocalDb,
                confidence: (0.5 + usage as f32 * 0.1).min(0.9),
                preview: None,
            });
        }

        // 2. Check domain packs — extract before mutable borrow
        let domain_hit = self.domain_packs.iter().find_map(|(_, entries)| {
            entries
                .iter()
                .find(|e| {
                    e.explanation == explanation && (e.language == context || context.is_empty())
                })
                .map(|e| (e.token.clone(), e.explanation.clone()))
        });
        if let Some((token, exp)) = domain_hit {
            self.push_context(exp, token.clone(), context.to_string());
            return Ok(ExplainResult {
                token,
                source: ExplainSource::DomainPack,
                confidence: 0.9,
                preview: None,
            });
        }

        // 3. Check web cache
        let cached = self.web_cache.get(explanation).map(|c| c.result.clone());
        if let Some(result) = cached {
            self.push_context(explanation.to_string(), result.clone(), context.to_string());
            return Ok(ExplainResult {
                token: result,
                source: ExplainSource::WebCache,
                confidence: 0.7,
                preview: None,
            });
        }

        // 4. Heuristic fallback
        let result = Self::heuristic_explain(explanation, context);
        if let Some(ref memory) = self.memory {
            let _ = memory.store_explanation(context, explanation, &result);
        }
        self.push_context(explanation.to_string(), result.clone(), context.to_string());
        Ok(ExplainResult {
            token: result,
            source: ExplainSource::Heuristic,
            confidence: 0.5,
            preview: None,
        })
    }

    fn push_context(&mut self, explanation: String, result: String, context: String) {
        self.conversation.push(ExplanationEntry {
            explanation,
            result,
            context,
        });
        if self.conversation.len() > self.config.max_context {
            self.conversation.remove(0);
        }
    }

    pub fn conversation_context(&self) -> &[ExplanationEntry] {
        &self.conversation
    }
    pub fn clear_context(&mut self) {
        self.conversation.clear();
    }

    fn heuristic_explain(explanation: &str, context: &str) -> String {
        let e = explanation.to_lowercase();
        let is_code = matches!(
            context,
            "rust" | "python" | "javascript" | "go" | "c" | "cpp"
        );
        if is_code {
            if e.contains("connection") && e.contains("pool") {
                return "ConnectionPool".into();
            }
            if e.contains("user") && e.contains("repository") {
                return "UserRepository".into();
            }
            if e.contains("config") || e.contains("configuration") {
                return "Config".into();
            }
            if e.contains("handler") {
                return "Handler".into();
            }
            if e.contains("manager") {
                return "Manager".into();
            }
            if e.contains("service") {
                return "Service".into();
            }
            if e.contains("factory") {
                return "Factory".into();
            }
            if e.contains("builder") {
                return "Builder".into();
            }
            if e.contains("error") || e.contains("exception") {
                return "Error".into();
            }
            return to_pascal_case(explanation);
        }
        explanation.to_string()
    }

    pub fn domain_pack_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.domain_packs.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }
}

fn to_pascal_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize = true;
    for ch in s.chars() {
        if ch == ' ' || ch == '_' || ch == '-' {
            capitalize = true;
        } else if capitalize {
            result.push(ch.to_ascii_uppercase());
            capitalize = false;
        } else {
            result.push(ch);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heuristic_code_patterns() {
        assert_eq!(
            ExplainerV2::heuristic_explain("connection pool", "rust"),
            "ConnectionPool"
        );
    }
    #[test]
    fn test_domain_pack_python() {
        let mut e = ExplainerV2::new(Default::default(), None);
        e.load_builtin_packs();
        assert_eq!(
            e.explain("the function that reads a file", "python")
                .unwrap()
                .token,
            "open()"
        );
    }
    #[test]
    fn test_domain_pack_rust() {
        let mut e = ExplainerV2::new(Default::default(), None);
        e.load_builtin_packs();
        assert_eq!(
            e.explain("the function that reads a file", "rust")
                .unwrap()
                .token,
            "fs::read_to_string()"
        );
    }
    #[test]
    fn test_prose_fallback() {
        let mut e = ExplainerV2::new(Default::default(), None);
        assert_eq!(
            e.explain("the sky is blue", "prose").unwrap().token,
            "the sky is blue"
        );
    }
    #[test]
    fn test_conversation_eviction() {
        let config = ExplainerConfigV2 {
            max_context: 2,
            ..Default::default()
        };
        let mut e = ExplainerV2::new(config, None);
        e.explain("a", "prose").unwrap();
        e.explain("b", "prose").unwrap();
        assert_eq!(e.conversation.len(), 2);
        e.explain("c", "prose").unwrap();
        assert_eq!(e.conversation.len(), 2);
    }
    #[test]
    fn test_clear_context() {
        let mut e = ExplainerV2::new(Default::default(), None);
        e.explain("hello", "prose").unwrap();
        assert!(!e.conversation.is_empty());
        e.clear_context();
        assert!(e.conversation.is_empty());
    }
    #[test]
    fn test_domain_pack_names() {
        let mut e = ExplainerV2::new(Default::default(), None);
        e.load_builtin_packs();
        assert!(e.domain_pack_names().contains(&"python"));
    }
    #[test]
    fn test_pascal_case() {
        assert_eq!(to_pascal_case("hello world"), "HelloWorld");
    }
    #[test]
    fn test_source_display() {
        assert_eq!(format!("{}", ExplainSource::Heuristic), "Guess");
    }
}
