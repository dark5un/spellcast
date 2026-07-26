// SPDX-License-Identifier: Apache-2.0

//! Phonetic prediction engine v2.
//!
//! Improves on the basic phonetic predictor with:
//! - Phoneme-level edit distance with weighted operations
//! - Context-aware re-ranking using n-gram frequency tables
//! - Confidence-based prediction display
//! - User-adaptive correction model (lightweight ML from SQLite data)

use std::collections::HashMap;

use crate::tokenizer::TokenContext;

/// A prediction candidate with confidence.
#[derive(Debug, Clone)]
pub struct PredictionV2 {
    /// The predicted token text.
    pub token: String,
    /// Confidence score (0.0 - 1.0).
    pub confidence: f32,
    /// Phoneme edit distance (lower = better).
    pub distance: f32,
    /// Context-specific frequency score.
    pub context_score: f32,
}

impl PredictionV2 {
    /// Combined score combining distance, confidence, and context.
    pub fn combined_score(&self) -> f32 {
        // Lower distance is better
        let distance_score = 1.0 / (1.0 + self.distance);
        // Weighted average: 40% distance, 30% confidence, 30% context
        0.4 * distance_score + 0.3 * self.confidence + 0.3 * self.context_score
    }
}

/// Phoneme similarity weights for edit distance calculation.
pub struct PhonemeWeights {
    /// Cost of vowel substitution (default: 0.5 — vowels easily misheard).
    pub vowel_substitution: f32,
    /// Cost of consonant substitution (default: 1.0).
    pub consonant_substitution: f32,
    /// Cost of substitution between similar phonemes (default: 0.3).
    /// e.g., /p/ ↔ /b/, /t/ ↔ /d/, /f/ ↔ /v/
    pub similar_substitution: f32,
    /// Cost of insertion/deletion (default: 1.5).
    pub indel: f32,
    /// Cost of matching phonemes (default: 0.0).
    pub match_cost: f32,
}

impl Default for PhonemeWeights {
    fn default() -> Self {
        Self {
            vowel_substitution: 0.5,
            consonant_substitution: 1.0,
            similar_substitution: 0.3,
            indel: 1.5,
            match_cost: 0.0,
        }
    }
}

/// Phoneme-level edit distance calculator.
pub struct PhonemeEditDistance {
    weights: PhonemeWeights,
}

impl PhonemeEditDistance {
    pub fn new(weights: PhonemeWeights) -> Self {
        Self { weights }
    }

    /// Calculate weighted edit distance between two phoneme sequences.
    pub fn distance(&self, a: &[&str], b: &[&str]) -> f32 {
        let n = a.len();
        let m = b.len();
        let mut dp = vec![vec![0.0f32; m + 1]; n + 1];

        for i in 0..=n {
            dp[i][0] = i as f32 * self.weights.indel;
        }
        for j in 0..=m {
            dp[0][j] = j as f32 * self.weights.indel;
        }

        for i in 1..=n {
            for j in 1..=m {
                let cost = if a[i - 1] == b[j - 1] {
                    self.weights.match_cost
                } else if Self::is_similar_phoneme(a[i - 1], b[j - 1]) {
                    self.weights.similar_substitution
                } else if Self::is_vowel(a[i - 1]) && Self::is_vowel(b[j - 1]) {
                    self.weights.vowel_substitution
                } else {
                    self.weights.consonant_substitution
                };

                dp[i][j] = (dp[i - 1][j] + self.weights.indel)
                    .min(dp[i][j - 1] + self.weights.indel)
                    .min(dp[i - 1][j - 1] + cost);
            }
        }

        dp[n][m]
    }

    /// Check if two phonemes are phonetically similar.
    fn is_similar_phoneme(a: &str, b: &str) -> bool {
        matches!(
            (a, b),
            ("p", "b") | ("b", "p")
                | ("t", "d") | ("d", "t")
                | ("k", "g") | ("g", "k")
                | ("f", "v") | ("v", "f")
                | ("s", "z") | ("z", "s")
                | ("ʃ", "ʒ") | ("ʒ", "ʃ")
                | ("θ", "ð") | ("ð", "θ")
                | ("m", "n") | ("n", "m")
        )
    }

    fn is_vowel(phoneme: &str) -> bool {
        matches!(phoneme, "a" | "e" | "i" | "o" | "u" | "ə" | "æ" | "ɛ" | "ɪ" | "ʊ" | "ɔ" | "ʌ")
    }
}

/// N-gram frequency tables for context-aware re-ranking.
pub struct ContextLanguageModel {
    /// Bigram frequencies: (prev_word, word) → count
    bigrams: HashMap<(String, String), u64>,
    /// Unigram frequencies: word → count
    unigrams: HashMap<String, u64>,
    /// Total word count.
    total: u64,
}

impl ContextLanguageModel {
    pub fn new() -> Self {
        Self {
            bigrams: HashMap::new(),
            unigrams: HashMap::new(),
            total: 0,
        }
    }

    /// Feed a token stream to update the language model.
    pub fn feed_line(&mut self, line: &str) {
        let mut prev: Option<String> = None;
        for word in line.split_whitespace() {
            let word = word.to_lowercase();
            *self.unigrams.entry(word.clone()).or_insert(0) += 1;
            if let Some(p) = prev {
                *self.bigrams.entry((p, word.clone())).or_insert(0) += 1;
            }
            prev = Some(word);
            self.total += 1;
        }
    }

    /// Get the probability of a word given the previous word.
    pub fn bigram_prob(&self, prev: &str, word: &str) -> f32 {
        let count = self
            .bigrams
            .get(&(prev.to_lowercase(), word.to_lowercase()))
            .copied()
            .unwrap_or(0);
        let prev_count = self.unigrams.get(&prev.to_lowercase()).copied().unwrap_or(1);
        count as f32 / prev_count as f32
    }

    /// Get the unigram probability (prior).
    pub fn unigram_prob(&self, word: &str) -> f32 {
        let count = self.unigrams.get(&word.to_lowercase()).copied().unwrap_or(0);
        if self.total == 0 {
            return 0.0;
        }
        count as f32 / self.total as f32
    }

    pub fn word_count(&self) -> u64 {
        self.total
    }
}

impl Default for ContextLanguageModel {
    fn default() -> Self {
        Self::new()
    }
}

/// The enhanced phonetic prediction engine.
pub struct PredictorV2 {
    /// Phoneme edit distance calculator.
    edit_distance: PhonemeEditDistance,
    /// Context language model for re-ranking.
    context_model: ContextLanguageModel,
    /// Phonetic encodings for known words.
    word_phonemes: HashMap<String, Vec<String>>,
    /// ASR confidence threshold for prediction display.
    asr_confidence_threshold: f32,
}

impl PredictorV2 {
    pub fn new() -> Self {
        Self {
            edit_distance: PhonemeEditDistance::new(PhonemeWeights::default()),
            context_model: ContextLanguageModel::new(),
            word_phonemes: HashMap::new(),
            asr_confidence_threshold: 0.7,
        }
    }

    /// Set the ASR confidence threshold.
    pub fn set_confidence_threshold(&mut self, threshold: f32) {
        self.asr_confidence_threshold = threshold;
    }

    /// Add a word with its phoneme encoding to the dictionary.
    pub fn add_word(&mut self, word: &str, phonemes: Vec<&str>) {
        self.word_phonemes.insert(
            word.to_lowercase(),
            phonemes.into_iter().map(|s| s.to_string()).collect(),
        );
    }

    /// Look up the phoneme sequence for a word.
    pub fn get_phonemes(&self, word: &str) -> Option<&Vec<String>> {
        self.word_phonemes.get(&word.to_lowercase())
    }

    /// Rank predictions for a spoken word using context.
    /// `spoken` is the raw ASR output text.
    /// `prev_word` is the previous token (for bigram context).
    /// `context` determines the domain (e.g., code vs prose).
    pub fn rank_predictions(
        &self,
        spoken: &str,
        prev_word: Option<&str>,
        _context: TokenContext,
    ) -> Vec<PredictionV2> {
        let spoken_phonemes = self.word_phonemes.get(&spoken.to_lowercase());
        let spoken_phonemes: Vec<&str> = spoken_phonemes
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_else(|| vec![spoken]);

        let mut predictions: Vec<PredictionV2> = self
            .word_phonemes
            .iter()
            .filter_map(|(word, phonemes)| {
                let p: Vec<&str> = phonemes.iter().map(|s| s.as_str()).collect();
                let dist = self.edit_distance.distance(&spoken_phonemes, &p);
                if dist > 5.0 {
                    return None; // Too far
                }

                let context_score = match prev_word {
                    Some(prev) => self.context_model.bigram_prob(prev, word),
                    None => self.context_model.unigram_prob(word),
                };

                Some(PredictionV2 {
                    token: word.clone(),
                    confidence: 1.0 / (1.0 + dist),
                    distance: dist,
                    context_score,
                })
            })
            .collect();

        // Sort by combined score (descending)
        predictions.sort_by(|a, b| {
            b.combined_score()
                .partial_cmp(&a.combined_score())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        predictions.truncate(5); // Top 5
        predictions
    }

    /// Determine whether to show predictions based on ASR confidence.
    pub fn prediction_display_mode(&self, asr_confidence: f32) -> PredictionDisplay {
        if asr_confidence > 0.9 {
            PredictionDisplay::Hidden // Confident enough, don't distract
        } else if asr_confidence > self.asr_confidence_threshold {
            PredictionDisplay::Dimmed // Show but dim
        } else {
            PredictionDisplay::Prominent // Show prominently
        }
    }

    /// Feed a correction into the context model for learning.
    pub fn learn_correction(&mut self, line: &str) {
        self.context_model.feed_line(line);
    }

    /// Get context model stats.
    pub fn context_stats(&self) -> (u64, usize) {
        (
            self.context_model.word_count(),
            self.word_phonemes.len(),
        )
    }
}

impl Default for PredictorV2 {
    fn default() -> Self {
        Self::new()
    }
}

/// How predictions should be displayed to the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PredictionDisplay {
    /// Don't show predictions (ASR was confident).
    Hidden,
    /// Show predictions with dimmed style.
    Dimmed,
    /// Show predictions with prominent style.
    Prominent,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phoneme_distance_identical() {
        let ed = PhonemeEditDistance::new(PhonemeWeights::default());
        let a = vec!["h", "ɛ", "l", "o"];
        let b = vec!["h", "ɛ", "l", "o"];
        assert!((ed.distance(&a, &b) - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_phoneme_distance_similar() {
        let ed = PhonemeEditDistance::new(PhonemeWeights::default());
        // "there" vs "their" — similar phonetically
        let a = vec!["ð", "ɛ", "r"];
        let b = vec!["ð", "ɛ", "r"];
        assert!((ed.distance(&a, &b) - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_phoneme_distance_different() {
        let ed = PhonemeEditDistance::new(PhonemeWeights::default());
        let a = vec!["h", "ɛ", "l", "o"];
        let b = vec!["g", "ɔ", "d", "b", "aɪ"];
        assert!(ed.distance(&a, &b) > 2.0);
    }

    #[test]
    fn test_similar_phonemes() {
        assert!(PhonemeEditDistance::is_similar_phoneme("p", "b"));
        assert!(PhonemeEditDistance::is_similar_phoneme("t", "d"));
        assert!(!PhonemeEditDistance::is_similar_phoneme("p", "t"));
    }

    #[test]
    fn test_context_model_bigram() {
        let mut model = ContextLanguageModel::new();
        model.feed_line("hello world hello everyone");
        let prob = model.bigram_prob("hello", "world");
        assert!(prob > 0.0);
        let prob2 = model.bigram_prob("hello", "unknown");
        assert_eq!(prob2, 0.0);
    }

    #[test]
    fn test_prediction_ranking() {
        let mut predictor = PredictorV2::new();
        // Add some words with phonemes
        predictor.add_word("hello", vec!["h", "ɛ", "l", "o"]);
        predictor.add_word("helmet", vec!["h", "ɛ", "l", "m", "ɛ", "t"]);
        predictor.add_word("help", vec!["h", "ɛ", "l", "p"]);
        predictor.add_word("world", vec!["w", "ɜr", "l", "d"]);

        let results = predictor.rank_predictions("hello", None, TokenContext::Prose);
        assert!(!results.is_empty());
        // "hello" should be the top match
        assert_eq!(results[0].token, "hello");
    }

    #[test]
    fn test_prediction_display_mode() {
        let predictor = PredictorV2::new();
        assert_eq!(
            predictor.prediction_display_mode(0.95),
            PredictionDisplay::Hidden
        );
        assert_eq!(
            predictor.prediction_display_mode(0.8),
            PredictionDisplay::Dimmed
        );
        assert_eq!(
            predictor.prediction_display_mode(0.5),
            PredictionDisplay::Prominent
        );
    }

    #[test]
    fn test_learn_correction() {
        let mut predictor = PredictorV2::new();
        predictor.learn_correction("the quick brown fox");
        let stats = predictor.context_stats();
        assert_eq!(stats.0, 4);
    }
}