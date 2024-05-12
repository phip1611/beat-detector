//! Benchmarks a few general audio transformations relevant in the field of this
//! crate. Useful to run this on a host platform to see the roughly costs.
//!
//! To run bench these, run `$ cargo bench "convert samples"`

use beat_detector::util::{f32_sample_to_i16, i16_sample_to_f32, stereo_to_mono};
use criterion::{criterion_group, criterion_main, Criterion};
use itertools::Itertools;
use std::hint::black_box;

fn criterion_benchmark(c: &mut Criterion) {
    let typical_sampling_rate = 44100;
    let sample_count = typical_sampling_rate;
    let mut samples_f32 = vec![0.0; sample_count];
    samples_f32.fill_with(rand::random::<f32>);
    let mut samples_i16 = vec![0; sample_count];
    samples_i16.fill_with(rand::random::<i16>);

    assert_eq!(samples_f32.len(), sample_count);
    assert_eq!(samples_i16.len(), sample_count);

    c.bench_function(
        &format!("{sample_count} convert samples (i16 to f32)"),
        |b| {
            b.iter(|| {
                let _res = black_box(
                    samples_i16
                        .iter()
                        .copied()
                        .map(|s| i16_sample_to_f32(black_box(s)))
                        .collect::<Vec<_>>(),
                );
            })
        },
    );

    c.bench_function(
        &format!("{sample_count} convert samples (i16 to f32 (just cast))"),
        |b| {
            b.iter(|| {
                let _res = black_box(
                    samples_i16
                        .iter()
                        .copied()
                        .map(|s| black_box(s as f32))
                        .collect::<Vec<_>>(),
                );
            })
        },
    );

    c.bench_function(
        &format!("{sample_count} convert samples (f32 to i16)"),
        |b| {
            b.iter(|| {
                let _res = black_box(
                    samples_f32
                        .iter()
                        .copied()
                        .map(|s| f32_sample_to_i16(black_box(s)).unwrap())
                        .collect::<Vec<_>>(),
                );
            })
        },
    );

    c.bench_function(
        &format!("{sample_count} convert samples (i16 stereo to mono)"),
        |b| {
            b.iter(|| {
                let _res = black_box(
                    samples_i16
                        .iter()
                        .copied()
                        // We pretend the data is interleaved (LRLR pattern).
                        .chunks(2)
                        .into_iter()
                        .map(|mut lr| {
                            let l = lr.next().unwrap();
                            let r = lr
                                .next()
                                .expect("should have an even number of LRLR samples");
                            stereo_to_mono(black_box(l), black_box(r))
                        })
                        .collect::<Vec<_>>(),
                );
            })
        },
    );
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
