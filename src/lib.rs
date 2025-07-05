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
//! beat-detector detects beats in live audio, but can also be used for post
//! analysis of audio data. It is a library written in Rust that is
//! `no_std`-compatible and doesn't need `alloc`.
//!
//! beat-detector was developed with typical sampling rates and bit depths in
//! mind, namely 44.1 kHz, 48.0 kHz, and 16 bit. Other input sources might work
//! as well.
//!
//!
//! ## TL;DR
//!
//! Use [`BeatDetector`].
//!
//! ## Audio Source
//!
//! The library operates on `i16` mono-channel samples. There are public helpers
//! that might assist you preparing the audio material for the crate:
//!
//! - [`util::f32_sample_to_i16`]
//! - [`util::stereo_to_mono`]
//!
//! ## Example
//!
//! ```rust
//! use beat_detector::BeatDetector;
//! let mono_samples = [0, 500, -800, 700 /*, ... */];
//! let mut detector = BeatDetector::new(44100.0, false);
//!
//! let is_beat = detector.update_and_detect_beat(
//!     mono_samples.iter().copied()
//! );
//! ```
//!
//! ## Detection and Usage
//!
//! The beat detector is supposed to be continuously invoked with the latest
//! audio samples. On each invocation, it checks if the internal audio buffer
//! contains a beat. The same beat won't be reported multiple times.
//!
//! The detector should be regularly fed with samples that are only
//! a fraction of the internal buffer, For live analysis, ~20ms per invocation
//! are fine. For post analysis, this property is not too important.
//!
//! However, the new audio samples should never be more than what the internal
//! buffer can hold, otherwise you might lose beats.
//!
//! ### Audio Source
//!
//! The audio source must have a certain amount of power. Very low values are
//! considered as noise and are not taken into account. But you need also to
//! prevent clipping! Ideally, you check your audio source with the "Record"
//! feature of Audacity or a similar tool visually, so that you can limit
//! potential sources of error.
//!
//! ## Detection Strategy
//!
//! The beat detection strategy is **not** based on state-of-the-art scientific
//! research, but on a best-effort approach and common sense.
//!
//! ## Technical Information
//!
//! beat-detector uses a smart chaining of iterators in different abstraction
//! levels to minimize buffering. In that process, it tries to never iterate
//! data multiple times, if not necessary, to keep the latency low.

#![no_std]
#![deny(
    clippy::all,
    clippy::cargo,
    clippy::nursery,
    // clippy::restriction,
    // clippy::pedantic
)]
// now allow a few rules which are denied by the above statement
// --> they are ridiculous and not necessary
#![allow(
    clippy::suboptimal_flops,
    clippy::redundant_pub_crate,
    clippy::fallible_impl_from,
    clippy::multiple_crate_versions
)]
#![deny(missing_debug_implementations)]
#![deny(rustdoc::all)]

extern crate alloc;
#[cfg_attr(any(test, feature = "std"), macro_use)]
#[cfg(any(test, feature = "std"))]
extern crate std;

// Better drop-in replacement for "assert!" and even better "check!" macro.
#[cfg_attr(test, macro_use)]
#[cfg(test)]
extern crate assert2;

#[cfg_attr(test, macro_use)]
#[cfg(test)]
extern crate float_cmp;

mod audio_preprocessing;
#[cfg(feature = "std")]
mod audio_io;
mod audio_analysis;
// mod max_min_iterator;
/// PRIVATE. For tests and helper binaries.
#[cfg(test)]
mod test_utils;
// use max_min_iterator::MaxMinIterator;

#[cfg(todo)]
mod tests {
    use super::*;
    use crate::layer_analysis::audio_history::AudioHistory;
    use crate::max_min_iterator::MaxMinIterator;
    use crate::test_utils;
    use std::vec::Vec;

    fn _print_sample_stats((samples, header): (Vec<i16>, hound::WavSpec)) {
        let sample_rate = header.sample_rate as f32;
        let sample_rate = sample_rate.try_into().unwrap();
        let mut history = AudioHistory::new(sample_rate, None);
        history.update(samples.iter().copied());

        let all_peaks = MaxMinIterator::new(&history, None).collect::<Vec<_>>();

        let abs_peak_value_iter = all_peaks.iter().map(|info| info.amplitude.abs());

        let max: i16 = abs_peak_value_iter.clone().max().unwrap();
        let min: i16 = abs_peak_value_iter.clone().min().unwrap();

        let avg: i16 =
            (abs_peak_value_iter.map(|v| v as u64).sum::<u64>() / all_peaks.len() as u64) as i16;

        let mut all_peaks_sorted = all_peaks.clone();
        all_peaks_sorted.sort_by(|a, b| a.amplitude.abs().partial_cmp(&b.amplitude.abs()).unwrap());

        let median: i16 = all_peaks_sorted[all_peaks_sorted.len() / 2].amplitude.abs();

        eprintln!("max abs peak     : {max:.3}");
        eprintln!("min abs peak     : {min:.3}");
        eprintln!("average abs peak : {avg:.3}");
        eprintln!("median abs peak  : {median:.3}");
        eprintln!("max / avg peak   : {:.3}", max / avg);
        eprintln!("max / median peak: {:.3}", max / median);
        eprintln!(
            "peaks abs        : {:#.3?}",
            all_peaks
                .iter()
                .map(|info| info.amplitude.abs())
                .collect::<Vec<_>>()
        );
        eprintln!(
            "peak next_to_curr ratio: {:#.3?}",
            all_peaks
                .iter()
                .zip(all_peaks.iter().skip(1))
                .map(|(current, next)| { next.amplitude.abs() / current.amplitude.abs() })
                .collect::<Vec<_>>()
        );
    }

    /// This just prints a few statistics of the used sample. This helps to
    /// understand characteristics of certain properties in a sample, such as
    /// the characteristic of an envelope.
    #[test]
    fn print_holiday_single_beat_stats() {
        eprintln!("holiday stats (single beat):");
        _print_sample_stats(test_utils::samples::holiday_single_beat())
    }

    /// This just prints a few statistics of the used sample. This helps to
    /// understand characteristics of certain properties in a sample, such as
    /// the characteristic of an envelope.
    #[test]
    fn print_sample1_single_beat_stats() {
        eprintln!("sample1 stats (single beat):");
        _print_sample_stats(test_utils::samples::sample1_single_beat())
    }
}
