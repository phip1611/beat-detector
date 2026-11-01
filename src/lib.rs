// SPDX-License-Identifier: MIT

//! beat-detector detects beats in live audio and in recordings. It is
//! `no_std`-compatible, doesn't allocate, and keeps only a few hundred bytes
//! of state.
//!
//! ## Live Audio
//!
//! Pass each new buffer of the audio input to [`BeatDetector::process`]:
//!
//! ```rust
//! use beat_detector::BeatDetector;
//!
//! let mut detector = BeatDetector::new(44100.0);
//! // In the callback of your audio input:
//! let buffer = [0.0_f32; 256];
//! if let Some(beat) = detector.process(&buffer) {
//!     println!("beat at {:?}", beat.time);
//! }
//! ```
//!
//! With the `recording` feature, [`recording::start_detector_thread`] does
//! this for the audio input of the system.
//!
//! ## Recordings
//!
//! [`detect_all`] returns the beats of a complete recording, e.g., to compare
//! them with the waveform in an audio editor such as Audacity:
//!
//! ```rust
//! let samples = vec![0.0_f32; 44100];
//! for beat in beat_detector::detect_all(&samples, 44100.0) {
//!     println!("beat at {:?}", beat.time);
//! }
//! ```
//!
//! ## Audio Input
//!
//! The detector expects mono `f32` samples in range `-1.0..=1.0`. Convert
//! `i16` samples with `f32::from(sample) / 32768.0` and mix stereo channels
//! by averaging them. The volume doesn't matter much: the detector adapts to
//! it.
//!
//! ## Detection Strategy
//!
//! The detector reports sudden rises in the level of the bass, i.e., kick
//! drums, a few milliseconds after their onset. It is not based on
//! state-of-the-art research but aims to be simple and understandable. See
//! [`BeatDetector`] for the details.

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

mod detector;
#[cfg(feature = "std")]
mod stdlib;
#[cfg(test)]
mod test_utils;

pub use detector::{Beat, BeatDetector, Config, detect_all};
#[cfg(feature = "std")]
pub use stdlib::*;
