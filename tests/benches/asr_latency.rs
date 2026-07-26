// SPDX-License-Identifier: Apache-2.0

//! ASR latency benchmarks.

use criterion::{Criterion, black_box, criterion_group, criterion_main};

fn benchmark_asr_pipeline(_c: &mut Criterion) {
    // TODO: Implement ASR latency benchmark when ASR engine is configured.
    // This requires a model file to be present.
    #[cfg(feature = "test-asr")]
    {
        use voxkey::asr::NoopAsr;
        use voxkey::audio::MockAudioCapture;

        let audio = MockAudioCapture::new().generate_test_buffer(3.0);
        let asr = NoopAsr::new();

        _c.bench_function("asr_transcribe_noop", |b| {
            b.iter(|| {
                let result = asr.transcribe(black_box(&audio)).unwrap();
                black_box(result);
            });
        });
    }
}

fn benchmark_audio_conversion(c: &mut Criterion) {
    use voxkey::audio::AudioBuffer;

    let samples: Vec<i16> = (0..48000).map(|i| (i as f32 * 0.5).sin() as i16).collect();

    c.bench_function("audio_to_f32", |b| {
        let buf = AudioBuffer {
            samples: samples.clone(),
            sample_rate: 16000,
        };
        b.iter(|| {
            let result = buf.to_f32();
            black_box(result);
        });
    });
}

criterion_group! {
    name = asr;
    config = Criterion::default().sample_size(10);
    targets = benchmark_asr_pipeline, benchmark_audio_conversion
}

criterion_main!(asr);
