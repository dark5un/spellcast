// SPDX-License-Identifier: Apache-2.0

//! Audio capture module.
//!
//! Uses `cpal` to capture audio from the microphone.
//! Provides push-to-talk recording (start/stop) and returns
//! 16kHz mono 16-bit PCM buffers suitable for Whisper.

#[allow(dead_code)]
pub mod vad;

use crate::error::{SpellcastError, SpellcastResult};
use std::sync::Arc;
use std::sync::Mutex;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// Audio capture configuration.
#[derive(Debug, Clone)]
pub struct AudioConfig {
    /// Sample rate in Hz (default: 16000)
    pub sample_rate: u32,
    /// Number of channels (default: 1)
    pub channels: u16,
    /// Audio device name, or "default"
    pub device: String,
}

/// A recorded audio buffer with metadata.
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    /// PCM samples (16-bit, mono, 16kHz)
    pub samples: Vec<i16>,
    /// Sample rate of the recording
    pub sample_rate: u32,
}

impl AudioBuffer {
    /// Convert to f32 samples (for whisper-rs).
    pub fn to_f32(&self) -> Vec<f32> {
        self.samples.iter().map(|&s| s as f32 / 32768.0).collect()
    }

    /// Duration in seconds.
    pub fn duration_seconds(&self) -> f64 {
        self.samples.len() as f64 / self.sample_rate as f64
    }
}

/// Audio capture device abstraction.
pub struct AudioCapture {
    config: AudioConfig,
    _device: cpal::Device,
    _supported_config: cpal::SupportedStreamConfig,
}

impl AudioCapture {
    /// Create a new audio capture instance.
    ///
    /// Opens the specified audio device and selects the appropriate
    /// stream configuration (16kHz, mono, 16-bit).
    pub fn new(config: &AudioConfig) -> SpellcastResult<Self> {
        let host = cpal::default_host();

        let device = if config.device == "default" {
            host.default_input_device()
                .ok_or_else(|| SpellcastError::Audio("No default input device found".to_string()))?
        } else {
            host.input_devices()
                .map_err(|e| SpellcastError::Audio(format!("Failed to list devices: {e}")))?
                .find(|d| {
                    d.description()
                        .map(|desc| desc.name() == config.device)
                        .unwrap_or(false)
                })
                .ok_or_else(|| {
                    SpellcastError::Audio(format!("Audio device '{}' not found", config.device))
                })?
        };

        let description = device
            .description()
            .map_err(|e| SpellcastError::Audio(format!("Failed to get device description: {e}")))?;

        let supported_config = device
            .default_input_config()
            .map_err(|e| SpellcastError::Audio(format!("Failed to query input configs: {e}")))?;

        log::info!(
            "Audio device: {} ({:?})",
            description.name(),
            supported_config
        );

        Ok(Self {
            config: config.clone(),
            _device: device,
            _supported_config: supported_config,
        })
    }

    /// Record audio for the given duration in seconds.
    ///
    /// Blocks the current thread for the duration of the recording.
    /// Returns the audio buffer.
    pub fn record_duration(&self, duration_secs: f64) -> SpellcastResult<AudioBuffer> {
        let sample_rate = self.config.sample_rate;
        let _channels = self.config.channels as usize;
        let samples_needed = (sample_rate as f64 * duration_secs) as usize;

        let samples = Arc::new(Mutex::new(Vec::with_capacity(samples_needed)));
        let samples_clone = Arc::clone(&samples);

        let err_channel = Arc::new(Mutex::new(None::<String>));
        let err_clone = Arc::clone(&err_channel);

        let stream: cpal::platform::Stream = self
            ._device
            .build_input_stream::<f32, _, _>(
                self._supported_config.config(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let mut guard = samples_clone.lock().unwrap();
                    for &sample in data {
                        guard.push((sample * 32768.0) as i16);
                    }
                },
                move |err| {
                    let mut guard = err_clone.lock().unwrap();
                    *guard = Some(format!("Audio stream error: {err}"));
                },
                None,
            )
            .map_err(|e| SpellcastError::Audio(format!("Failed to build input stream: {e}")))?;

        // Start the stream
        stream
            .play()
            .map_err(|e| SpellcastError::Audio(format!("Failed to start audio stream: {e}")))?;

        // Record for the specified duration
        let recording_duration = if duration_secs > 0.0 {
            duration_secs
        } else {
            3.0
        };
        std::thread::sleep(std::time::Duration::from_secs_f64(recording_duration));

        // Stop the stream
        drop(stream);

        // Check for errors during recording
        if let Some(err) = err_channel.lock().unwrap().take() {
            return Err(SpellcastError::Audio(err));
        }

        let guard = samples.lock().unwrap();
        let pcm_samples = guard.clone();
        drop(guard);

        if pcm_samples.is_empty() {
            return Err(SpellcastError::Audio("No audio captured".to_string()));
        }

        // Convert from device sample rate to 16kHz if needed
        let resampled = if sample_rate != 16000 {
            // Simple linear resampling — good enough for MVP
            resample_linear(&pcm_samples, sample_rate, 16000)
        } else {
            pcm_samples
        };

        log::info!(
            "Captured {} samples ({:.1}s)",
            resampled.len(),
            resampled.len() as f64 / 16000.0
        );

        Ok(AudioBuffer {
            samples: resampled,
            sample_rate: if sample_rate != 16000 {
                16000
            } else {
                sample_rate
            },
        })
    }

    /// Record audio using push-to-talk: start recording, return when stopped.
    ///
    /// For the MVP, this records a fixed duration.
    /// A future version will use VAD (voice activity detection) or a key toggle.
    pub fn record_push_to_talk(&self, timeout_secs: f64) -> SpellcastResult<AudioBuffer> {
        // For MVP: record for a fixed duration (push-to-talk via key to start/stop)
        self.record_duration(timeout_secs)
    }
}

/// Simple linear resampling.
fn resample_linear(input: &[i16], from_rate: u32, to_rate: u32) -> Vec<i16> {
    if from_rate == to_rate {
        return input.to_vec();
    }
    let ratio = from_rate as f64 / to_rate as f64;
    let output_len = (input.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_pos = i as f64 * ratio;
        let src_idx = src_pos as usize;
        let frac = src_pos - src_idx as f64;

        if src_idx + 1 < input.len() {
            let sample = input[src_idx] as f64 * (1.0 - frac) + input[src_idx + 1] as f64 * frac;
            output.push(sample as i16);
        } else {
            output.push(*input.last().unwrap_or(&0));
        }
    }

    output
}

/// Mock audio capture for testing.
pub struct MockAudioCapture;

impl Default for MockAudioCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl MockAudioCapture {
    pub fn new() -> Self {
        Self
    }

    /// Generate a synthetic sine wave buffer for testing.
    pub fn generate_test_buffer(&self, duration_secs: f64) -> AudioBuffer {
        let sample_rate = 16000;
        let num_samples = (sample_rate as f64 * duration_secs) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f64 / sample_rate as f64;
            // 440 Hz sine wave at 30% amplitude
            let sample = (t * 440.0 * 2.0 * std::f64::consts::PI).sin() * 0.3 * 32768.0;
            samples.push(sample as i16);
        }

        AudioBuffer {
            samples,
            sample_rate,
        }
    }

    pub fn record_duration(&self, _duration_secs: f64) -> SpellcastResult<AudioBuffer> {
        Ok(self.generate_test_buffer(_duration_secs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resample_linear_identity() {
        let input = vec![100i16, 200, 300, 400];
        let output = resample_linear(&input, 16000, 16000);
        assert_eq!(input, output);
    }

    #[test]
    fn test_audio_buffer_conversion() {
        let buf = AudioBuffer {
            samples: vec![0i16, 16384, 32767, -16384, -32768],
            sample_rate: 16000,
        };
        let f32_buf = buf.to_f32();
        assert_eq!(f32_buf.len(), 5);
        assert!((f32_buf[0] - 0.0).abs() < 0.001);
        assert!((f32_buf[2] - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_audio_buffer_duration() {
        let buf = AudioBuffer {
            samples: vec![0i16; 16000],
            sample_rate: 16000,
        };
        assert!((buf.duration_seconds() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_mock_capture() {
        let mock = MockAudioCapture::new();
        let buf = mock.generate_test_buffer(1.0);
        assert_eq!(buf.sample_rate, 16000);
        assert_eq!(buf.samples.len(), 16000);
    }
}
