/*
MIT License

Copyright (c) 2024 Philipp Schuster

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
*/

use crate::audio_preprocessing::conversion::stereo_to_mono;
use itertools::Itertools;
use std::path::{Path, PathBuf};
use std::vec::Vec;

/// Reads a WAV file to mono audio. Returns the samples as mono audio.
/// Additionally, it returns the sampling rate of the file.
fn read_wav_to_mono<T: AsRef<Path>>(file: T) -> (Vec<i16>, hound::WavSpec) {
    let mut reader = hound::WavReader::open(file).unwrap();
    let header = reader.spec();

    // owning vector with original data in i16 format
    let data = reader
        .samples::<i16>()
        .map(|s| s.unwrap())
        .collect::<Vec<_>>();

    if header.channels == 1 {
        (data, header)
    } else if header.channels == 2 {
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

/// Returns the cargo target dir.
pub fn target_dir() -> PathBuf {
    // 1. Check if CARGO_TARGET_DIR is set
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        PathBuf::from(dir)
    } else {
        // 2. Fall back to default: go up from CARGO_MANIFEST_DIR
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest_dir.join("target")
    }
}
/// Returns a directory within the cargo target dir to store test artifacts.
pub fn target_dir_test_artifacts() -> PathBuf {
    let mut path = target_dir();
    path.push("test_generated");
    path
}

/// Accessor to various samples. One sample here refers to what a sample is in
/// the music industry: A small excerpt of audio. "Samples" however refer to the
/// individual data points.
pub mod samples {
    use super::*;
    use crate::audio_preprocessing::audio_history::MIN_WINDOW;

    /// Returns the mono samples of the holiday sample (long version)
    /// together with the sampling rate.
    pub fn holiday_long() -> (Vec<i16>, hound::WavSpec) {
        read_wav_to_mono("res/holiday_lowpassed--long.wav")
    }

    /// Returns the mono samples of the holiday sample (excerpt version)
    /// together with the sampling rate.
    pub fn holiday_excerpt() -> (Vec<i16>, hound::WavSpec) {
        read_wav_to_mono("res/holiday_lowpassed--excerpt.wav")
    }

    /// Returns the mono samples of the holiday sample (single-beat version)
    /// together with the sampling rate.
    pub fn holiday_single_beat() -> (Vec<i16>, hound::WavSpec) {
        read_wav_to_mono("res/holiday_lowpassed--single-beat.wav")
    }

    /// Returns the mono samples of the "sample1" sample (long version)
    /// together with the sampling rate.
    pub fn sample1_long() -> (Vec<i16>, hound::WavSpec) {
        read_wav_to_mono("res/sample1_lowpassed--long.wav")
    }

    /// Returns the mono samples of the "sample1" sample (single-beat version)
    /// together with the sampling rate.
    pub fn sample1_single_beat() -> (Vec<i16>, hound::WavSpec) {
        read_wav_to_mono("res/sample1_lowpassed--single-beat.wav")
    }

    /// Returns the mono samples of the "sample1" sample (double-beat version)
    /// together with the sampling rate.
    pub fn sample1_double_beat() -> (Vec<i16>, hound::WavSpec) {
        read_wav_to_mono("res/sample1_lowpassed--double-beat.wav")
    }

    #[test]
    fn test_samples_are_as_long_as_expected() {
        fn to_duration_in_seconds((samples, header): (Vec<i16>, hound::WavSpec)) -> f32 {
            // Although my code is generic regarding the sampling rate, in my
            // demo samples, I only use this sampling rate. So let's do a
            // sanity check.
            assert_eq!(header.sample_rate, 44100);

            samples.len() as f32 / header.sample_rate as f32
        }

        let duration = to_duration_in_seconds(holiday_excerpt());
        assert_eq!(duration, 0.035804987 /* seconds */);
        assert!(
            duration * 1000.0 <= MIN_WINDOW.as_millis() as f32,
            "All test code relies on that this sample fully fits into the audio window!"
        );

        let duration = to_duration_in_seconds(holiday_long());
        assert_eq!(duration, 3.1764627 /* seconds */);

        let duration = to_duration_in_seconds(holiday_single_beat());
        assert_eq!(duration, 0.40773243 /* seconds */);
        assert!(
            duration * 1000.0 <= MIN_WINDOW.as_millis() as f32,
            "All test code relies on that this sample fully fits into the audio window!"
        );

        let duration = to_duration_in_seconds(sample1_long());
        assert_eq!(duration, 7.998526 /* seconds */);

        let duration = to_duration_in_seconds(sample1_single_beat());
        assert_eq!(duration, 0.18380952 /* seconds */);
        assert!(
            duration * 1000.0 <= MIN_WINDOW.as_millis() as f32,
            "All test code relies on that this sample fully fits into the audio window!"
        );

        let duration = to_duration_in_seconds(sample1_double_beat());
        assert_eq!(duration, 0.41687074 /* seconds */);
        assert!(
            duration * 1000.0 <= MIN_WINDOW.as_millis() as f32,
            "All test code relies on that this sample fully fits into the audio window!"
        );
    }
}
