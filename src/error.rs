// SPDX-License-Identifier: Apache-2.0

//! Error types for the VoxKey system.
//!
//! A unified error type `VoxKeyError` with variants for each subsystem,
//! plus automatic conversion from common error types.

use thiserror::Error;

/// Top-level error type for all VoxKey operations.
#[derive(Error, Debug)]
pub enum VoxKeyError {
    // -- Config errors --
    /// Config file not found or unreadable.
    #[error("config error: {0}")]
    Config(String),

    /// Config file failed to parse.
    #[error("config parse error: {0}")]
    ConfigParse(#[from] toml::de::Error),

    // -- Audio errors --
    /// Audio device not found or failed to open stream.
    #[error("audio error: {0}")]
    Audio(String),

    /// Audio capture stream failed.
    #[error("audio stream error: {0}")]
    AudioStream(#[from] Box<dyn std::error::Error + Send + Sync>),

    // -- ASR errors --
    /// ASR model failed to load.
    #[error("ASR model error: {0}")]
    AsrModel(String),

    /// ASR inference failed.
    #[error("ASR inference error: {0}")]
    AsrInference(String),

    // -- Tokenizer errors --
    /// Tokenization failed.
    #[error("tokenizer error: {0}")]
    Tokenizer(String),

    // -- Predictor errors --
    /// Phonetic prediction failed.
    #[error("predictor error: {0}")]
    Predictor(String),

    // -- Explainer errors --
    /// Explainer DB lookup failed.
    #[error("explainer DB error: {0}")]
    ExplainerDb(String),

    /// LLM inference failed.
    #[error("LLM error: {0}")]
    Llm(String),

    /// Web search failed.
    #[error("web search error: {0}")]
    WebSearch(String),

    // -- Database errors --
    /// Database initialization or query failed.
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    // -- Terminal errors --
    /// PTY creation or management failed.
    #[error("terminal PTY error: {0}")]
    TerminalPty(String),

    /// Terminal rendering failed.
    #[error("terminal render error: {0}")]
    TerminalRender(String),

    // -- Backend errors --
    /// Compute backend initialization failed.
    #[error("backend error: {0}")]
    Backend(String),

    // -- I/O errors --
    /// General I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    // -- General errors --
    /// Unexpected internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Convenience result type alias.
pub type VoxKeyResult<T> = Result<T, VoxKeyError>;

impl From<String> for VoxKeyError {
    fn from(s: String) -> Self {
        VoxKeyError::Internal(s)
    }
}

impl From<&str> for VoxKeyError {
    fn from(s: &str) -> Self {
        VoxKeyError::Internal(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = VoxKeyError::Config("missing file".to_string());
        assert_eq!(err.to_string(), "config error: missing file");
    }

    #[test]
    fn test_error_from_string() {
        let err: VoxKeyError = "something broke".into();
        assert!(matches!(err, VoxKeyError::Internal(_)));
    }

    #[test]
    fn test_error_from_str() {
        let err: VoxKeyError = VoxKeyError::from("test error");
        assert!(matches!(err, VoxKeyError::Internal(_)));
    }

    #[test]
    fn test_audio_error() {
        let err = VoxKeyError::Audio("no device found".to_string());
        assert_eq!(err.to_string(), "audio error: no device found");
    }

    #[test]
    fn test_backend_error() {
        let err = VoxKeyError::Backend("CUDA unavailable".to_string());
        assert_eq!(err.to_string(), "backend error: CUDA unavailable");
    }
}
