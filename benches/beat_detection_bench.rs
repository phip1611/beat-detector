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
    pub fn holiday_long() -> (Vec<f32>, wav::Header) {
        read_wav_to_mono("res/holiday_lowpassed--long.wav")
    }
}

mod helpers {
    use std::fs::File;
    use std::path::Path;

    fn i16_sample_to_f32_sample(val: i16) -> f32 {
        if val == 0 {
            0.0
        } else {
            val as f32 / i16::MAX as f32
        }
    }

    /// Reads a WAV file to mono audio. Returns the samples as mono audio.
    /// Additionally, it returns the sampling rate of the file.
    pub fn read_wav_to_mono<T: AsRef<Path>>(file: T) -> (Vec<f32>, wav::Header) {
        let mut file = File::open(file).unwrap();
        let (header, data) = wav::read(&mut file).unwrap();

        // owning vector with original data in f32 format
        let original_data_f32 = if data.is_sixteen() {
            data.as_sixteen()
                .unwrap()
                .iter()
                .map(|sample| i16_sample_to_f32_sample(*sample))
                .collect()
        } else if data.is_thirty_two_float() {
            data.as_thirty_two_float().unwrap().clone()
        } else {
            panic!("unsupported format");
        };

        assert!(
            !original_data_f32.iter().any(|&x| libm::fabsf(x) > 1.0),
            "float audio data must be in interval [-1, 1]."
        );

        if header.channel_count == 1 {
            (original_data_f32, header)
        } else if header.channel_count == 2 {
            let mut mono_audio = Vec::new();
            for sample in original_data_f32.chunks(2) {
                let mono_sample = (sample[0] + sample[1]) / 2.0;
                mono_audio.push(mono_sample);
            }
            (mono_audio, header)
        } else {
            panic!("unsupported format!");
        }
    }
}
