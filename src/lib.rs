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
//! beat-detector is a `no_std`-compatible and alloc-free library written in
//! Rust for detecting beats in audio. It can be used for both post- and live
//! detection.
//!
//! ## Audio Source
//!
//! All it needs to fulfill its work are audio samples in value range
//! `[-1.0..=1.0]` and the sampling rate. Audio samples are expected to be in
//! mono channel format.
//!
//! ## Detection and Usage
//!
//! The beat detector is supposed to be continuously invoked with the latest
//! audio samples from the audio source in time frames that must be less than
//! the amount of buffered audio history. As soon as a beat is found in the
//! internally buffered audio history, this is reported, and the same beat won't
//! be reported multiple times.
//!
//! The audio source must have a certain amount of power. Very low peaks are
//! considered as noise and are not taken into account.
//!
//! ### Strategy
//!
//! The beat detection strategy is not based on state-of-the-art scientific
//! research but on a best-effort and common sense.
//!
//! ## Technical Information
//!
//! This library doesn't need any allocations or buffering of data in
//! `heapless`-like data structures. Instead, beat-detector uses a smart
//! chaining of multiple iterators in different abstraction levels to find
//! beats:
//! ```raw
//! - Beat Detector     --uses-->
//! - Envelope Iterator --uses-->
//! - MaxMin Iterator   --uses-->
//! - Root Iterator
//! ```

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

#[cfg_attr(any(test, feature = "std"), macro_use)]
#[cfg(any(test, feature = "std"))]
extern crate std;

mod audio_history;
mod beat_detector;
mod envelope_iterator;
mod max_min_iterator;
mod root_iterator;
#[cfg(feature = "std")]
mod stdlib;
/// PRIVATE. For tests and helper binaries.
#[cfg(test)]
mod test_utils;

pub use audio_history::{AudioHistory, SampleInfo};
pub use beat_detector::{AudioInput, BeatDetector, BeatInfo, StubIterator};
pub use envelope_iterator::{EnvelopeInfo, EnvelopeIterator};
#[cfg(feature = "std")]
pub use std::*;

use max_min_iterator::MaxMinIterator;
use root_iterator::RootIterator;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio_history::AudioHistory;
    use crate::max_min_iterator::MaxMinIterator;
    use crate::test_utils;
    use std::vec::Vec;

    fn _print_sample_stats((samples, header): (Vec<f32>, wav::Header)) {
        let mut history = AudioHistory::new(header.sampling_rate as f32);
        history.update(samples.iter().copied());

        let all_peaks = MaxMinIterator::new(&history, None).collect::<Vec<_>>();

        let abs_peak_value_iter = all_peaks.iter().map(|info| info.value_abs);

        let max: f32 = abs_peak_value_iter
            .clone()
            .reduce(|a, b| if a > b { a } else { b })
            .unwrap();

        let min: f32 = abs_peak_value_iter
            .clone()
            .reduce(|a, b| if a > b { b } else { a })
            .unwrap();

        let avg: f32 = abs_peak_value_iter.reduce(|a, b| a + b).unwrap() / all_peaks.len() as f32;

        let mut all_peaks_sorted = all_peaks.clone();
        all_peaks_sorted.sort_by(|a, b| a.value_abs.partial_cmp(&b.value_abs).unwrap());

        let median: f32 = all_peaks_sorted[all_peaks_sorted.len() / 2].value_abs;

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
                .map(|info| info.value_abs)
                .collect::<Vec<_>>()
        );
        eprintln!(
            "peak next_to_curr ratio: {:#.3?}",
            all_peaks
                .iter()
                .zip(all_peaks.iter().skip(1))
                .map(|(current, next)| { next.value_abs / current.value_abs })
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
