// SPDX-License-Identifier: Apache-2.0

//! Integration tests for the audio pipeline.

use voxkey::audio::{AudioBuffer, MockAudioCapture};
use voxkey::asr::{AsrEngine, NoopAsr};

#[test]
fn test_audio_to_asr_pipeline() {
    let mock_audio = MockAudioCapture::new();
    let audio = mock_audio.generate_test_buffer(3.0);

    // Audio buffer should have the expected properties
    assert_eq!(audio.sample_rate, 16000);
    assert!(!audio.samples.is_empty());
    assert!((audio.duration_seconds() - 3.0).abs() < 0.1);

    // Convert to f32 (as needed by ASR)
    let f32_samples = audio.to_f32();
    assert_eq!(f32_samples.len(), audio.samples.len());

    // Run through noop ASR
    let asr = NoopAsr::new();
    let result = asr.transcribe(&audio).unwrap();
    assert_eq!(result.text, "test transcription");
}

#[test]
fn test_multiple_recordings() {
    let mock = MockAudioCapture::new();

    for duration in [0.5, 1.0, 2.0] {
        let buf = mock.generate_test_buffer(duration);
        let expected_samples = (duration * 16000.0) as usize;
        assert_eq!(buf.samples.len(), expected_samples);
        assert!((buf.duration_seconds() - duration).abs() < 0.1);
    }
}