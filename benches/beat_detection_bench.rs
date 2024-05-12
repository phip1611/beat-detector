use beat_detector::BeatDetector;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn criterion_benchmark(c: &mut Criterion) {
    let (samples, header) = samples::holiday_long();
    // Chosen a value in the middle with lots of peaks, so lots of calculations
    // to be done.
    let slice_of_interest = &samples[28000..28000 + 4096];

    let mut detector = BeatDetector::new(header.sampling_rate as f32, true);
    c.bench_function(
        "simulate beat detection (with lowpass) with 4096 samples per invocation",
        |b| {
            b.iter(|| {
                // We do not care about the correct detection. Using this, I just want
                // to find out overall calculation time and do profiling to see which
                // functions can be optimized.
                let _ =
                    detector.update_and_detect_beat(black_box(slice_of_interest.iter().copied()));
            })
        },
    );

    let mut detector = BeatDetector::new(header.sampling_rate as f32, false);
    c.bench_function(
        "simulate beat detection (no lowpass) with 4096 samples per invocation",
        |b| {
            b.iter(|| {
                // We do not care about the correct detection. Using this, I just want
                // to find out overall calculation time and do profiling to see which
                // functions can be optimized.
                let _ =
                    detector.update_and_detect_beat(black_box(slice_of_interest.iter().copied()));
            })
        },
    );
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

mod samples {
    use crate::helpers::read_wav_to_mono;

    /// Returns the mono samples of the holiday sample (long version)
    /// together with the sampling rate.
    pub fn holiday_long() -> (Vec<i16>, wav::Header) {
        read_wav_to_mono("res/holiday_lowpassed--long.wav")
    }
}

/// Copy and paste from `test_utils.rs`.
mod helpers {
    use beat_detector::util::{f32_sample_to_i16, stereo_to_mono};
    use itertools::Itertools;
    use std::fs::File;
    use std::path::Path;
    use wav::BitDepth;

    pub fn read_wav_to_mono<T: AsRef<Path>>(file: T) -> (Vec<i16>, wav::Header) {
        let mut file = File::open(file).unwrap();
        let (header, data) = wav::read(&mut file).unwrap();

        // owning vector with original data in f32 format
        let data = match data {
            BitDepth::Sixteen(samples) => samples,
            BitDepth::ThirtyTwoFloat(samples) => samples
                .into_iter()
                .map(f32_sample_to_i16)
                .map(Result::unwrap)
                .collect::<Vec<_>>(),
            _ => todo!("{data:?} not supported yet"),
        };

        if header.channel_count == 1 {
            (data, header)
        } else if header.channel_count == 2 {
            let data = data
                .into_iter()
                .chunks(2)
                .into_iter()
                .map(|mut lr| {
                    let l = lr.next().unwrap();
                    let r = lr
                        .next()
                        .expect("should have an even number of LRLR samples");
                    stereo_to_mono(l, r)
                })
                .collect::<Vec<_>>();
            (data, header)
        } else {
            panic!("unsupported format!");
        }
    }
}
