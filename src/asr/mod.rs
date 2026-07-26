// SPDX-License-Identifier: Apache-2.0

//! ASR (Automatic Speech Recognition) engine abstraction.
//!
//! Defines the `AsrEngine` trait and provides implementations:
//! - `WhisperAsr` — backed by whisper.cpp via `whisper-rs`
//! - `NoopAsr` — mock/stub for testing

use crate::audio::AudioBuffer;
use crate::error::{SpellcastError, SpellcastResult};

/// Result of ASR inference.
#[derive(Debug, Clone)]
pub struct AsrResult {
    /// The transcribed text.
    pub text: String,
    /// Confidence score (0.0 - 1.0), if available.
    pub confidence: Option<f32>,
    /// Duration of the inference in milliseconds.
    pub inference_ms: u64,
}

impl AsrResult {
    /// Create a new ASR result.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            confidence: None,
            inference_ms: 0,
        }
    }

    /// Set the confidence score.
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = Some(confidence);
        self
    }
}

/// Trait for ASR (speech-to-text) engines.
pub trait AsrEngine: Send {
    /// Load a model from the given path.
    fn load_model(&mut self, model_path: &str) -> SpellcastResult<()>;

    /// Transcribe an audio buffer to text.
    fn transcribe(&self, audio: &AudioBuffer) -> SpellcastResult<AsrResult>;

    /// Check if the engine is ready for inference.
    fn is_ready(&self) -> bool;
}

/// Whisper ASR implementation using whisper.cpp via whisper-rs.
#[cfg(feature = "whisper-rs")]
pub struct WhisperAsr {
    ctx: Option<whisper_rs::WhisperContext>,
    model_path: String,
    language: String,
}

#[cfg(feature = "whisper-rs")]
impl WhisperAsr {
    /// Create a new Whisper ASR instance.
    pub fn new(model_path: &str, _backend: &str) -> SpellcastResult<Self> {
        let ctx = Self::load_context(model_path)?;
        Ok(Self {
            ctx: Some(ctx),
            model_path: model_path.to_string(),
            language: "en".to_string(),
        })
    }

    fn load_context(model_path: &str) -> SpellcastResult<whisper_rs::WhisperContext> {
        let params = whisper_rs::WhisperContextParameters::default();
        whisper_rs::WhisperContext::new_with_params(model_path, params)
            .map_err(|e| SpellcastError::AsrModel(format!("Failed to load Whisper model: {e}")))
    }
}

#[cfg(feature = "whisper-rs")]
impl AsrEngine for WhisperAsr {
    fn load_model(&mut self, model_path: &str) -> SpellcastResult<()> {
        self.ctx = Some(Self::load_context(model_path)?);
        self.model_path = model_path.to_string();
        Ok(())
    }

    fn transcribe(&self, audio: &AudioBuffer) -> SpellcastResult<AsrResult> {
        let ctx = self
            .ctx
            .as_ref()
            .ok_or_else(|| SpellcastError::AsrInference("Model not loaded".to_string()))?;

        let start = std::time::Instant::now();

        // Create a state (requires &mut)
        let mut state = ctx
            .create_state()
            .map_err(|e| SpellcastError::AsrInference(format!("Failed to create state: {e}")))?;

        // Convert audio to f32
        let audio_f32 = audio.to_f32();

        // Run full transcription
        let mut params =
            whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 5 });

        // Set language
        params.set_language(Some(&self.language));
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_no_timestamps(true);

        state
            .full(params, &audio_f32)
            .map_err(|e| SpellcastError::AsrInference(format!("Transcription failed: {e}")))?;

        // Collect text from all segments using the iterator
        let mut text = String::new();
        for segment in state.as_iter() {
            let seg_text = segment
                .to_str()
                .map_err(|e| SpellcastError::AsrInference(e.to_string()))?;
            text.push_str(seg_text);
            text.push(' ');
        }

        let elapsed = start.elapsed().as_millis() as u64;

        log::info!("ASR inference: {elapsed}ms, text: '{text}'");

        Ok(AsrResult {
            text: text.trim().to_string(),
            confidence: None,
            inference_ms: elapsed,
        })
    }

    fn is_ready(&self) -> bool {
        self.ctx.is_some()
    }
}

/// No-op ASR engine for testing when whisper-rs is unavailable.
pub struct NoopAsr;

impl Default for NoopAsr {
    fn default() -> Self {
        Self::new()
    }
}

impl NoopAsr {
    pub fn new() -> Self {
        Self
    }
}

impl AsrEngine for NoopAsr {
    fn load_model(&mut self, _model_path: &str) -> SpellcastResult<()> {
        Ok(())
    }

    fn transcribe(&self, _audio: &AudioBuffer) -> SpellcastResult<AsrResult> {
        Ok(AsrResult::new("test transcription"))
    }

    fn is_ready(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asr_result() {
        let r = AsrResult::new("hello world");
        assert_eq!(r.text, "hello world");
        assert!(r.confidence.is_none());
    }

    #[test]
    fn test_asr_result_with_confidence() {
        let r = AsrResult::new("test").with_confidence(0.95);
        assert_eq!(r.confidence, Some(0.95));
    }

    #[test]
    fn test_noop_asr() {
        let engine = NoopAsr::new();
        assert!(engine.is_ready());
        let buf = AudioBuffer {
            samples: vec![0i16; 16000],
            sample_rate: 16000,
        };
        let result = engine.transcribe(&buf).unwrap();
        assert_eq!(result.text, "test transcription");
    }

    #[test]
    fn test_noop_load_model() {
        let mut engine = NoopAsr::new();
        assert!(engine.load_model("nonexistent").is_ok());
    }

    #[test]
    fn test_noop_is_ready() {
        let engine = NoopAsr::new();
        assert!(engine.is_ready());
    }
}
