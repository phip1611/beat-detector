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
use std::fs::File;
use std::path::Path;
use std::vec::Vec;

fn i16_sample_to_f32_sample(val: i16) -> f32 {
    if val == 0 {
        0.0
    } else {
        val as f32 / i16::MAX as f32
    }
}

/// Reads a WAV file to mono audio. Returns the samples as mono audio.
/// Additionally, it returns the sampling rate of the file.
fn read_wav_to_mono<T: AsRef<Path>>(file: T) -> (Vec<f32>, wav::Header) {
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

/// Accessor to various samples. One sample here refers to what a sample is in
/// the music industry: A small excerpt of audio. "Samples" however refer to the
/// individual data points.
pub mod samples {
    use super::*;
    use crate::audio_history::DEFAULT_AUDIO_HISTORY_WINDOW_MS;

    /// Returns the mono samples of the holiday sample (long version)
    /// together with the sampling rate.
    pub fn holiday_long() -> (Vec<f32>, wav::Header) {
        read_wav_to_mono("res/holiday_lowpassed--long.wav")
    }

    /// Returns the mono samples of the holiday sample (excerpt version)
    /// together with the sampling rate.
    pub fn holiday_excerpt() -> (Vec<f32>, wav::Header) {
        read_wav_to_mono("res/holiday_lowpassed--excerpt.wav")
    }

    /// Returns the mono samples of the holiday sample (single-beat version)
    /// together with the sampling rate.
    pub fn holiday_single_beat() -> (Vec<f32>, wav::Header) {
        read_wav_to_mono("res/holiday_lowpassed--single-beat.wav")
    }

    /// Returns the mono samples of the "sample1" sample (long version)
    /// together with the sampling rate.
    pub fn sample1_long() -> (Vec<f32>, wav::Header) {
        read_wav_to_mono("res/sample1_lowpassed--long.wav")
    }

    /// Returns the mono samples of the "sample1" sample (single-beat version)
    /// together with the sampling rate.
    pub fn sample1_single_beat() -> (Vec<f32>, wav::Header) {
        read_wav_to_mono("res/sample1_lowpassed--single-beat.wav")
    }

    /// Returns the mono samples of the "sample1" sample (double-beat version)
    /// together with the sampling rate.
    pub fn sample1_double_beat() -> (Vec<f32>, wav::Header) {
        read_wav_to_mono("res/sample1_lowpassed--double-beat.wav")
    }

    #[test]
    fn test_samples_are_as_long_as_expected() {
        fn to_duration_in_seconds((samples, header): (Vec<f32>, wav::Header)) -> f32 {
            // Although my code is generic regarding the sampling rate, in my
            // demo samples, I only use this sampling rate. So let's do a
            // sanity check.
            assert_eq!(header.sampling_rate, 44100);

            samples.len() as f32 / header.sampling_rate as f32
        }

        let duration = to_duration_in_seconds(holiday_excerpt());
        assert_eq!(duration, 0.035804987 /* seconds */);
        assert!(
            duration * 1000.0 <= DEFAULT_AUDIO_HISTORY_WINDOW_MS as f32,
            "All test code relies on that this sample fully fits into the audio window!"
        );

        let duration = to_duration_in_seconds(holiday_long());
        assert_eq!(duration, 3.1764627 /* seconds */);

        let duration = to_duration_in_seconds(holiday_single_beat());
        assert_eq!(duration, 0.40773243 /* seconds */);
        assert!(
            duration * 1000.0 <= DEFAULT_AUDIO_HISTORY_WINDOW_MS as f32,
            "All test code relies on that this sample fully fits into the audio window!"
        );

        let duration = to_duration_in_seconds(sample1_long());
        assert_eq!(duration, 7.998526 /* seconds */);

        let duration = to_duration_in_seconds(sample1_single_beat());
        assert_eq!(duration, 0.18380952 /* seconds */);
        assert!(
            duration * 1000.0 <= DEFAULT_AUDIO_HISTORY_WINDOW_MS as f32,
            "All test code relies on that this sample fully fits into the audio window!"
        );

        let duration = to_duration_in_seconds(sample1_double_beat());
        assert_eq!(duration, 0.41687074 /* seconds */);
        assert!(
            duration * 1000.0 <= DEFAULT_AUDIO_HISTORY_WINDOW_MS as f32,
            "All test code relies on that this sample fully fits into the audio window!"
        );
    }
}
