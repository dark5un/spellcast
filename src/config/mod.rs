// SPDX-License-Identifier: Apache-2.0

//! Configuration loading, validation, and default generation.
//!
//! Configuration is loaded from a TOML file at `~/.config/voxkey/config.toml`.
//! Missing files fall back to defaults; malformed files return an error.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{VoxKeyError, VoxKeyResult};

/// Backend configuration: which compute backend to use.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum BackendType {
    /// Auto-detect: CUDA → Vulkan → CPU
    #[serde(alias = "auto")]
    #[default]
    Auto,
    /// NVIDIA CUDA
    #[serde(alias = "cuda")]
    Cuda,
    /// Vulkan (not yet implemented in MVP)
    #[serde(alias = "vulkan")]
    Vulkan,
    /// CPU-only fallback
    #[serde(alias = "cpu")]
    Cpu,
}

impl std::fmt::Display for BackendType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendType::Auto => write!(f, "auto"),
            BackendType::Cuda => write!(f, "cuda"),
            BackendType::Vulkan => write!(f, "vulkan"),
            BackendType::Cpu => write!(f, "cpu"),
        }
    }
}

/// Audio capture configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    /// Sample rate in Hz (default: 16000 for Whisper)
    pub sample_rate: u32,
    /// Number of audio channels (default: 1 for mono)
    pub channels: u16,
    /// Audio device name, or "default" for system default
    pub device: String,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            channels: 1,
            device: "default".to_string(),
        }
    }
}

/// ASR (Automatic Speech Recognition) engine configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrConfig {
    /// Engine type: "whisper-cpp", "sherpa-onnx"
    pub engine: String,
    /// Path to the ASR model file
    pub model_path: String,
    /// Language code for recognition (e.g., "en")
    pub language: String,
}

impl Default for AsrConfig {
    fn default() -> Self {
        Self {
            engine: "whisper-cpp".to_string(),
            model_path: "~/.config/voxkey/models/ggml-base.en.bin".to_string(),
            language: "en".to_string(),
        }
    }
}

/// LLM configuration for the explain feature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Engine type: "mistral-rs", "none"
    pub engine: String,
    /// Path or HuggingFace ID for the model
    pub model_path: String,
    /// Maximum tokens to generate
    pub max_tokens: u32,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            engine: "mistral-rs".to_string(),
            model_path: "Qwen/Qwen3-4B".to_string(),
            max_tokens: 50,
        }
    }
}

/// Keybinding configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyConfig {
    pub mode_toggle: String,
    pub caps_toggle: String,
    pub prev_token: String,
    pub next_token: String,
    pub redictate: String,
    pub delete_token: String,
    pub explain: String,
    pub kill_switch: String,
    pub accept_prediction_1: String,
    pub accept_prediction_2: String,
    pub accept_prediction_3: String,
}

impl Default for KeyConfig {
    fn default() -> Self {
        Self {
            mode_toggle: "CapsLock".to_string(),
            caps_toggle: "Shift+CapsLock".to_string(),
            prev_token: "h".to_string(),
            next_token: "l".to_string(),
            redictate: "r".to_string(),
            delete_token: "x".to_string(),
            explain: "e".to_string(),
            kill_switch: "Ctrl+Shift+Escape".to_string(),
            accept_prediction_1: "1".to_string(),
            accept_prediction_2: "2".to_string(),
            accept_prediction_3: "3".to_string(),
        }
    }
}

/// Tokenizer configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenizerConfig {
    /// Tokenizer mode: "heuristic" or "tree-sitter"
    pub mode: String,
    /// Default context: "prose" or "code"
    pub default_context: String,
}

impl Default for TokenizerConfig {
    fn default() -> Self {
        Self {
            mode: "heuristic".to_string(),
            default_context: "prose".to_string(),
        }
    }
}

/// Database configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Path to the SQLite database file
    pub path: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: "~/.config/voxkey/voxkey.db".to_string(),
        }
    }
}

/// Language configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageConfig {
    pub primary: String,
    pub secondary: String,
}

impl Default for LanguageConfig {
    fn default() -> Self {
        Self {
            primary: "en".to_string(),
            secondary: "none".to_string(),
        }
    }
}

/// Top-level VoxKey configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendConfig {
    /// Compute backend type
    #[serde(rename = "type")]
    pub backend_type: BackendType,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            backend_type: BackendType::Auto,
        }
    }
}

/// Top-level VoxKey configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VoxKeyConfig {
    #[serde(default)]
    pub backend: BackendConfig,
    #[serde(default)]
    pub audio: AudioConfig,
    #[serde(default)]
    pub asr: AsrConfig,
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub keys: KeyConfig,
    #[serde(default)]
    pub tokenizer: TokenizerConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub languages: LanguageConfig,
}

/// Load configuration from a TOML file.
///
/// Returns default configuration if the file doesn't exist.
/// Returns an error if the file exists but cannot be parsed.
pub fn load_config(path: &Path) -> VoxKeyResult<VoxKeyConfig> {
    let expanded = shellexpand::tilde(&path.to_string_lossy()).to_string();
    let config_path = Path::new(&expanded);

    if !config_path.exists() {
        log::info!("Config file {:?} not found, using defaults", config_path);
        return Ok(VoxKeyConfig::default());
    }

    let contents =
        std::fs::read_to_string(config_path).map_err(|e| VoxKeyError::Config(e.to_string()))?;
    let config: VoxKeyConfig = toml::from_str(&contents)?;
    Ok(config)
}

/// Generate the default configuration as a TOML string.
pub fn generate_default_config() -> String {
    let config = VoxKeyConfig::default();
    toml::to_string_pretty(&config).expect("default config should serialize")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_default_config() {
        let config = VoxKeyConfig::default();
        assert_eq!(config.audio.sample_rate, 16000);
        assert_eq!(config.asr.engine, "whisper-cpp");
        assert_eq!(config.keys.mode_toggle, "CapsLock");
        assert_eq!(config.tokenizer.mode, "heuristic");
    }

    #[test]
    fn test_load_config_missing_file() {
        let path = Path::new("/nonexistent/path/config.toml");
        let result = load_config(path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_load_config_invalid_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "invalid toml [[[").unwrap();
        drop(f);

        let result = load_config(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_config_valid_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let toml_content = r#"
[backend]
type = "cuda"

[audio]
sample_rate = 16000
channels = 1
device = "default"

[asr]
engine = "whisper-cpp"
model_path = "~/.config/voxkey/models/ggml-base.en.bin"
language = "en"
"#;
        std::fs::write(&path, toml_content).unwrap();
        let config = load_config(&path).unwrap();
        assert!(matches!(config.backend.backend_type, BackendType::Cuda));
    }

    #[test]
    fn test_generate_default_config_is_valid() {
        let toml_str = generate_default_config();
        let config: VoxKeyConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(config.audio.sample_rate, 16000);
    }
}
