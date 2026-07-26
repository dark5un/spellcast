// SPDX-License-Identifier: Apache-2.0

//! Voice Activity Detection (VAD) using Silero VAD.
//!
//! Wraps the silero-vad-rust crate for speech/non-speech segmentation.
//! Supports both offline (batch) and streaming chunk-based detection.
//! VAD support, continuous capture, and barge-in.

/// Configuration for the VAD engine.
#[derive(Debug, Clone)]
pub struct VadConfig {
    /// Sample rate (must be 8000, 16000, or 48000 for Silero).
    pub sample_rate: u32,
    /// Chunk size for streaming (samples). Silero expects multiples of 512/16000*sr.
    pub chunk_size: usize,
    /// Probability threshold for speech (0.0-1.0). Lower = more sensitive.
    pub threshold: f32,
    /// Minimum silence duration (ms) to end a speech segment.
    pub min_silence_ms: u32,
    /// Minimum speech duration (ms) to start a segment (filters noise bursts).
    pub min_speech_ms: u32,
    /// Pre-speech padding (ms) to include before speech starts.
    pub pre_padding_ms: u32,
    /// Post-speech padding (ms) to include after speech ends.
    pub post_padding_ms: u32,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            chunk_size: 512, // 32ms at 16kHz
            threshold: 0.5,
            min_silence_ms: 500,
            min_speech_ms: 100,
            pre_padding_ms: 100,
            post_padding_ms: 200,
        }
    }
}

/// A detected speech segment (timestamp range in samples).
#[derive(Debug, Clone)]
pub struct SpeechSegment {
    pub start_sample: usize,
    pub end_sample: usize,
}

/// VAD engine wrapping Silero VAD.
pub struct VoiceActivityDetector {
    config: VadConfig,
    #[cfg(feature = "vad")]
    model: Option<silero_vad_rust::silero_vad::model::SileroVadModel>,
    #[cfg(not(feature = "vad"))]
    _model: (),
}

impl VoiceActivityDetector {
    /// Create a new VAD instance. Loads the ONNX model on first use.
    pub fn new(config: VadConfig) -> Self {
        #[cfg(feature = "vad")]
        let model = Self::load_model();
        #[cfg(not(feature = "vad"))]
        let _model = ();

        Self {
            config,
            #[cfg(feature = "vad")]
            model: model.ok(),
            #[cfg(not(feature = "vad"))]
            _model,
        }
    }

    /// Load the Silero VAD ONNX model.
    #[cfg(feature = "vad")]
    fn load_model() -> Result<silero_vad_rust::silero_vad::model::SileroVadModel, anyhow::Error> {
        silero_vad_rust::load_silero_vad()
    }

    /// Process a single chunk of audio and return the speech probability.
    /// Audio must be 16kHz mono f32 samples.
    #[cfg(feature = "vad")]
    pub fn forward_chunk(&mut self, chunk: &[f32]) -> Option<f32> {
        let model = self.model.as_mut()?;
        let chunk_size = self.config.chunk_size;

        // Pad or truncate to expected chunk size
        let padded = if chunk.len() == chunk_size {
            chunk.to_vec()
        } else {
            let mut tmp = vec![0.0f32; chunk_size];
            let copy_len = chunk.len().min(chunk_size);
            tmp[..copy_len].copy_from_slice(&chunk[..copy_len]);
            tmp
        };

        let probs = model.forward_chunk(&padded, self.config.sample_rate).ok()?;
        Some(probs[[0, 0]])
    }

    /// Stub when VAD feature is disabled.
    #[cfg(not(feature = "vad"))]
    pub fn forward_chunk(&mut self, _chunk: &[f32]) -> Option<f32> {
        None
    }

    /// Detect speech segments in a full audio buffer (offline).
    #[cfg(feature = "vad")]
    pub fn detect_segments(&mut self, audio: &[f32]) -> Vec<SpeechSegment> {
        let model = match self.model.as_mut() {
            Some(m) => m,
            None => return vec![],
        };
        model.reset_states();

        let mut segments = Vec::new();
        let chunk_size = self.config.chunk_size;
        let mut in_speech = false;
        let mut segment_start = 0;
        let mut silence_frames = 0u32;
        let silence_threshold =
            self.config.min_silence_ms * self.config.sample_rate / 1000 / chunk_size as u32;
        let min_speech_frames =
            self.config.min_speech_ms * self.config.sample_rate / 1000 / chunk_size as u32;

        for (i, chunk) in audio.chunks(chunk_size).enumerate() {
            let padded = if chunk.len() == chunk_size {
                chunk.to_vec()
            } else {
                let mut tmp = vec![0.0f32; chunk_size];
                tmp[..chunk.len()].copy_from_slice(chunk);
                tmp
            };

            let prob = model.forward_chunk(&padded, self.config.sample_rate).ok();

            let is_speech = prob
                .map(|p| p[[0, 0]] >= self.config.threshold)
                .unwrap_or(false);

            if is_speech && !in_speech {
                in_speech = true;
                silence_frames = 0;
                // Apply pre-padding
                let pre_pad =
                    (self.config.pre_padding_ms * self.config.sample_rate / 1000) as usize;
                let pre_padded = if i * chunk_size > pre_pad {
                    i * chunk_size - pre_pad
                } else {
                    0
                };
                segment_start = pre_padded;
            } else if !is_speech && in_speech {
                silence_frames += 1;
                if silence_frames >= silence_threshold {
                    let frames = i - silence_frames as usize;
                    let end = (frames + 1) * chunk_size
                        + (self.config.post_padding_ms * self.config.sample_rate / 1000) as usize;
                    let end = end.min(audio.len());
                    if end > segment_start
                        && (end - segment_start) > (min_speech_frames * chunk_size) as usize
                    {
                        segments.push(SpeechSegment {
                            start_sample: segment_start,
                            end_sample: end,
                        });
                    }
                    in_speech = false;
                    silence_frames = 0;
                }
            } else if is_speech {
                silence_frames = 0;
            }
        }

        // Flush any residual speech segment
        if in_speech {
            let end = audio.len();
            if end > segment_start {
                segments.push(SpeechSegment {
                    start_sample: segment_start,
                    end_sample: end,
                });
            }
        }

        segments
    }

    /// Stub when VAD feature is disabled.
    #[cfg(not(feature = "vad"))]
    pub fn detect_segments(&mut self, _audio: &[f32]) -> Vec<SpeechSegment> {
        vec![]
    }

    /// Reset the VAD internal state (for streaming mode between segments).
    pub fn reset(&mut self) {
        #[cfg(feature = "vad")]
        if let Some(model) = &mut self.model {
            model.reset_states();
        }
    }

    /// Check if the VAD engine is loaded and ready.
    pub fn is_ready(&self) -> bool {
        #[cfg(feature = "vad")]
        {
            self.model.is_some()
        }
        #[cfg(not(feature = "vad"))]
        {
            false
        }
    }
}

/// A simple energy-based VAD as a fallback when neural VAD is unavailable.
/// Uses RMS energy with an adaptive threshold.
pub struct EnergyVad {
    sample_rate: u32,
    threshold: f32,
    chunk_size: usize,
}

impl EnergyVad {
    pub fn new(sample_rate: u32, threshold: f32, chunk_size: usize) -> Self {
        Self {
            sample_rate,
            threshold,
            chunk_size,
        }
    }

    /// Detect speech segments using RMS energy.
    pub fn detect_segments(&self, audio: &[f32]) -> Vec<SpeechSegment> {
        let mut segments = Vec::new();
        let mut in_speech = false;
        let mut segment_start = 0;

        for (i, chunk) in audio.chunks(self.chunk_size).enumerate() {
            let rms = (chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32).sqrt();
            let is_speech = rms > self.threshold;

            if is_speech && !in_speech {
                in_speech = true;
                segment_start = i * self.chunk_size;
            } else if !is_speech && in_speech {
                in_speech = false;
                segments.push(SpeechSegment {
                    start_sample: segment_start,
                    end_sample: i * self.chunk_size,
                });
            }
        }

        if in_speech {
            segments.push(SpeechSegment {
                start_sample: segment_start,
                end_sample: audio.len(),
            });
        }

        segments
    }
}

/// Continuous audio capture with VAD-based segmentation.
/// Maintains a ring buffer and yields speech segments.
#[allow(dead_code)]
pub struct ContinuousCapture {
    config: VadConfig,
    buffer: Vec<f32>,
    #[allow(dead_code)]
    sample_rate: u32,
}

impl ContinuousCapture {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            config: VadConfig::default(),
            buffer: Vec::with_capacity(sample_rate as usize * 10), // 10 second buffer
            sample_rate,
        }
    }

    /// Push audio samples into the capture buffer.
    pub fn push_samples(&mut self, samples: &[f32]) {
        self.buffer.extend_from_slice(samples);
        // Keep max 30 seconds of audio
        let max_samples = self.sample_rate as usize * 30;
        if self.buffer.len() > max_samples {
            self.buffer.drain(0..self.buffer.len() - max_samples);
        }
    }

    /// Extract the latest speech segment from the buffer.
    pub fn extract_segment(&mut self) -> Option<Vec<f32>> {
        let mut vad = VoiceActivityDetector::new(self.config.clone());
        let segments = vad.detect_segments(&self.buffer);

        if let Some(seg) = segments.last() {
            let segment: Vec<f32> = self.buffer[seg.start_sample..seg.end_sample].to_vec();
            // Clear consumed audio
            self.buffer.drain(0..seg.end_sample);
            Some(segment)
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

/// Barge-in buffer: accumulates audio while ASR is processing a previous segment.
/// When the ASR finishes, the buffered audio is immediately dispatched for transcription.
pub struct BargeInBuffer {
    buffer: Vec<f32>,
    sample_rate: u32,
}

impl BargeInBuffer {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            buffer: Vec::new(),
            sample_rate,
        }
    }

    /// Push audio while ASR is busy.
    pub fn push(&mut self, samples: &[f32]) {
        self.buffer.extend_from_slice(samples);
    }

    /// Drain the buffer for processing (returns audio and clears).
    pub fn drain(&mut self) -> Vec<f32> {
        let data = self.buffer.clone();
        self.buffer.clear();
        data
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vad_config_default() {
        let config = VadConfig::default();
        assert_eq!(config.sample_rate, 16000);
        assert_eq!(config.chunk_size, 512);
        assert!((config.threshold - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_vad_creates_without_panic() {
        let vad = VoiceActivityDetector::new(VadConfig::default());
        // Without VAD feature, is_ready should be false
        assert!(!vad.is_ready());
    }

    #[test]
    fn test_energy_vad_silence() {
        let vad = EnergyVad::new(16000, 0.1, 512);
        let silence = vec![0.0f32; 16000]; // 1 second of silence
        let segments = vad.detect_segments(&silence);
        assert!(segments.is_empty());
    }

    #[test]
    fn test_energy_vad_detects_speech() {
        let vad = EnergyVad::new(16000, 0.01, 512);
        let mut audio = vec![0.0f32; 16000];
        // Add a speech-like burst
        for i in 4000..8000 {
            audio[i] = 0.1 * (i as f32 * 0.01).sin();
        }
        let segments = vad.detect_segments(&audio);
        assert!(!segments.is_empty(), "should detect the speech burst");
    }

    #[test]
    fn test_continuous_capture_push_and_clear() {
        let mut capture = ContinuousCapture::new(16000);
        capture.push_samples(&[1.0f32; 512]);
        assert_eq!(capture.buffer.len(), 512);
        capture.clear();
        assert_eq!(capture.buffer.len(), 0);
    }

    #[test]
    fn test_barge_in_buffer() {
        let mut buf = BargeInBuffer::new(16000);
        assert!(buf.is_empty());
        buf.push(&[1.0f32; 256]);
        assert!(!buf.is_empty());
        assert_eq!(buf.len(), 256);

        let drained = buf.drain();
        assert_eq!(drained.len(), 256);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_barge_in_accumulates() {
        let mut buf = BargeInBuffer::new(16000);
        buf.push(&[1.0f32; 256]);
        buf.push(&[2.0f32; 256]);
        let drained = buf.drain();
        assert_eq!(drained.len(), 512);
    }
}
