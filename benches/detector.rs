// SPDX-License-Identifier: MIT

//! Measures the processing cost of the beat detector.
//!
//! Run with `cargo bench`. The interesting numbers:
//! - `throughput`: cost per sample when analyzing a recording
//! - `live_buffer`: cost per call with a typical live audio buffer

use beat_detector::BeatDetector;
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use std::f32::consts::TAU;
use std::hint::black_box;

const SAMPLE_RATE: f32 = 44100.0;

/// One second of kicks at 120 BPM on top of a bass tone and a high tone.
fn signal() -> Vec<f32> {
    (0..SAMPLE_RATE as usize)
        .map(|i| {
            let t = i as f32 / SAMPLE_RATE;
            let since_kick = t % 0.5;
            let kick = (-since_kick / 0.08).exp() * (TAU * 60.0 * since_kick).sin();
            0.6 * kick + 0.2 * (TAU * 55.0 * t).sin() + 0.1 * (TAU * 3000.0 * t).sin()
        })
        .collect()
}

fn bench(c: &mut Criterion) {
    let samples = signal();

    let mut group = c.benchmark_group("detector");
    group.throughput(Throughput::Elements(samples.len() as u64));
    group.bench_function("throughput", |b| {
        b.iter(|| {
            let mut detector = BeatDetector::new(SAMPLE_RATE);
            let chunk_len = detector.max_chunk_len();
            for chunk in black_box(&samples).chunks(chunk_len) {
                black_box(detector.process(chunk));
            }
        })
    });

    // 256 samples = 5.8 ms at 44.1 kHz.
    let buffer = &samples[..256];
    group.throughput(Throughput::Elements(buffer.len() as u64));
    let mut detector = BeatDetector::new(SAMPLE_RATE);
    group.bench_function("live_buffer", |b| {
        b.iter(|| black_box(detector.process(black_box(buffer))))
    });
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
