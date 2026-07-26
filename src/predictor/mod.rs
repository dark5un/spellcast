// SPDX-License-Identifier: Apache-2.0

//! Phonetic prediction engine.
//!
//! Provides phonetic similarity matching using Double Metaphone encoding.
//! After dictation, suggests up to 3 phonetically similar alternatives ranked
//! by edit distance on the phonetic code.

use std::collections::HashMap;

use rphonetic::{DoubleMetaphone, Encoder};

use crate::error::VoxKeyResult;

/// A phonetic prediction candidate.
#[derive(Debug, Clone)]
pub struct Prediction {
    /// The suggested word/token.
    pub word: String,
    /// The phonetic code used for matching.
    pub phonetic_code: String,
    /// Edit distance (lower is closer).
    pub distance: u32,
}

/// Phonetic prediction engine.
///
/// Uses Double Metaphone encoding and Levenshtein edit distance
/// to find phonetically similar words.
pub struct Predictor {
    /// Pre-computed phonetic index: word → phonetic code
    index: HashMap<String, String>,
    /// Double Metaphone encoder
    encoder: DoubleMetaphone,
}

impl Predictor {
    /// Create a new Predictor with an empty index.
    pub fn new() -> Self {
        Self {
            index: HashMap::new(),
            encoder: DoubleMetaphone::default(),
        }
    }

    /// Build a phonetic index from a word list.
    ///
    /// Common English words or domain-specific vocabulary.
    pub fn build_index<I, S>(&mut self, words: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for word in words {
            let code = self.encode(word.as_ref());
            self.index.insert(word.as_ref().to_lowercase(), code);
        }
        log::info!("Built phonetic index with {} entries", self.index.len());
    }

    /// Encode a word to its Double Metaphone representation.
    pub fn encode(&self, word: &str) -> String {
        // DoubleMetaphone returns primary and alternate codes
        // We use the primary code as the canonical encoding

        self.encoder.encode(word)
    }

    /// Find the top N phonetically similar words.
    ///
    /// Returns up to `count` predictions, excluding the input word itself,
    /// ranked by Levenshtein distance on the phonetic code.
    pub fn predict(&self, word: &str, count: usize) -> VoxKeyResult<Vec<Prediction>> {
        if self.index.is_empty() {
            return Ok(Vec::new());
        }

        let query_code = self.encode(word);
        let query_lower = word.to_lowercase();

        let mut candidates: Vec<Prediction> = self
            .index
            .iter()
            .filter(|(w, _)| *w != &query_lower)
            .map(|(w, code)| {
                let distance = levenshtein_distance(&query_code, code);
                Prediction {
                    word: w.clone(),
                    phonetic_code: code.clone(),
                    distance,
                }
            })
            .collect();

        // Sort by phonetic distance (ascending)
        candidates.sort_by_key(|p| p.distance);

        Ok(candidates.into_iter().take(count).collect())
    }

    /// Add a word to the index.
    pub fn add_word(&mut self, word: &str) {
        let code = self.encode(word);
        self.index.insert(word.to_lowercase(), code);
    }

    /// Get the size of the index.
    pub fn index_size(&self) -> usize {
        self.index.len()
    }
}

impl Default for Predictor {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute Levenshtein edit distance between two strings.
fn levenshtein_distance(a: &str, b: &str) -> u32 {
    let a_len = a.chars().count();
    let b_len = b.chars().count();

    if a_len == 0 {
        return b_len as u32;
    }
    if b_len == 0 {
        return a_len as u32;
    }

    let mut prev_row: Vec<u32> = (0..=b_len as u32).collect();
    let mut curr_row = vec![0u32; b_len + 1];

    for (i, ca) in a.chars().enumerate() {
        curr_row[0] = (i + 1) as u32;

        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr_row[j + 1] = std::cmp::min(
                std::cmp::min(curr_row[j] + 1, prev_row[j + 1] + 1),
                prev_row[j] + cost,
            );
        }

        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    prev_row[b_len]
}

/// Load common English words for the default phonetic index.
pub fn load_default_english_words() -> Vec<String> {
    // A small set of common English words for the MVP
    // In production, this would be loaded from a file
    vec![
        "the",
        "be",
        "to",
        "of",
        "and",
        "a",
        "in",
        "that",
        "have",
        "it",
        "for",
        "not",
        "on",
        "with",
        "he",
        "as",
        "you",
        "do",
        "at",
        "this",
        "but",
        "his",
        "by",
        "from",
        "they",
        "we",
        "her",
        "she",
        "or",
        "an",
        "will",
        "my",
        "one",
        "all",
        "would",
        "there",
        "their",
        "what",
        "so",
        "up",
        "out",
        "if",
        "about",
        "who",
        "get",
        "which",
        "go",
        "me",
        "when",
        "make",
        "can",
        "like",
        "time",
        "no",
        "just",
        "him",
        "know",
        "take",
        "people",
        "into",
        "year",
        "your",
        "good",
        "some",
        "could",
        "them",
        "see",
        "other",
        "than",
        "then",
        "now",
        "look",
        "only",
        "come",
        "its",
        "over",
        "think",
        "also",
        "back",
        "after",
        "use",
        "two",
        "how",
        "our",
        "work",
        "first",
        "well",
        "way",
        "even",
        "new",
        "want",
        "because",
        "any",
        "these",
        "give",
        "day",
        "most",
        "us",
        "function",
        "let",
        "var",
        "const",
        "return",
        "if",
        "else",
        "for",
        "while",
        "true",
        "false",
        "null",
        "undefined",
        "string",
        "number",
        "object",
        "array",
        "test",
        "run",
        "build",
        "error",
        "type",
        "struct",
        "enum",
        "impl",
        "trait",
        "pub",
        "fn",
        "mut",
    ]
    .into_iter()
    .map(|s| s.to_string())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_predictor() -> Predictor {
        let mut p = Predictor::new();
        p.build_index([
            "hello", "help", "held", "helm", "world", "word", "work", "there", "their", "they're",
            "wear", "where", "were",
        ]);
        p
    }

    #[test]
    fn test_phonetic_encoding() {
        let p = Predictor::new();
        let code = p.encode("hello");
        assert!(!code.is_empty());
    }

    #[test]
    fn test_similar_words_have_similar_codes() {
        let p = Predictor::new();
        let h1 = p.encode("there");
        let h2 = p.encode("their");
        let dist = levenshtein_distance(&h1, &h2);
        // "there" and "their" should be phonetically close
        assert!(dist <= 2, "there/their distance: {dist}");
    }

    #[test]
    fn test_predict_returns_similar_words() {
        let p = setup_predictor();
        let results = p.predict("there", 3).unwrap();
        assert!(!results.is_empty());
        assert!(results.len() <= 3);
        // Should find "their" as phonetically similar
        let words: Vec<&str> = results.iter().map(|r| r.word.as_str()).collect();
        assert!(words.contains(&"their") || words.contains(&"where"));
    }

    #[test]
    fn test_predict_excludes_input() {
        let mut p = Predictor::new();
        p.add_word("hello");
        let results = p.predict("hello", 3).unwrap();
        // With only one word in the index, this should return nothing
        assert!(results.is_empty());
    }

    #[test]
    fn test_empty_index_returns_empty() {
        let p = Predictor::new();
        let results = p.predict("hello", 3).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("", ""), 0);
        assert_eq!(levenshtein_distance("a", ""), 1);
        assert_eq!(levenshtein_distance("", "a"), 1);
        assert_eq!(levenshtein_distance("hello", "hello"), 0);
        assert_eq!(levenshtein_distance("hello", "hallo"), 1);
        assert_eq!(levenshtein_distance("hello", "world"), 4);
    }

    #[test]
    fn test_add_word() {
        let mut p = Predictor::new();
        assert_eq!(p.index_size(), 0);
        p.add_word("test");
        assert_eq!(p.index_size(), 1);
    }

    #[test]
    fn test_default_words_not_empty() {
        let words = load_default_english_words();
        assert!(!words.is_empty());
        assert!(words.len() > 100);
    }
}
