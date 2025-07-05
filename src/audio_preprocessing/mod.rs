//! Necessary types, helpers, and functions to pre-process audio input to
//! prepare it for the **analysis layer**.
//!
//! This module only operates on raw data and data streams, without interacting
//! with the outer world (I/O).

use crate::audio_preprocessing::conversion::{f32_sample_to_i16_unchecked, i16_sample_to_f32};
use crate::audio_preprocessing::downsampling::Downsampler;
use crate::audio_preprocessing::f32::F32Frequency;
use crate::audio_preprocessing::lowpass_filter::LowpassFilter;
use thiserror::Error;

pub mod audio_history;
pub mod conversion;
pub mod downsampling;
pub mod f32;
pub mod lowpass_filter;

/// Consumes an iterator over [`i16`] and produces a new iterator that processes
/// all samples by applying a lowpass filter and downsampling the samples
/// afterwards.
#[must_use]
#[inline]
pub fn lowpass_and_downsample_i16_samples_iter<'a>(
    samples: impl Iterator<Item = i16> + 'a,
    filter: &'a mut LowpassFilter,
    downsampler: &'a mut Downsampler,
) -> impl Iterator<Item = i16> + 'a {
    let iter = samples
        .map(i16_sample_to_f32)
        .map(|sample| filter.process(sample))
        // SAFETY: This is okay as we trust the biquad impl.
        // This code is on the hot path.
        .map(|sample| unsafe { f32_sample_to_i16_unchecked(sample) });

    downsampler.downsample(iter)
}

/// Possible errors when working with [`ValidInputFrequencies`].
#[derive(Debug, Clone, PartialEq, Error)]
pub enum InvalidFrequencyError {
    /// The cutoff frequency doesn't fulfill the Nyquist rule.
    #[error("invalid cutoff frequency: {0} * 2 <= sampling rate ({1}) is not ")]
    InvalidCutoffFrequency(F32Frequency, F32Frequency),
    #[error("the f32 value is not a valid frequency value")]
    InvalidF32(#[from] self::f32::InvalidF32Error),
}

/// Represents a validated pair of sample rate and cutoff frequency.
#[derive(Debug, Clone, Copy, PartialOrd, PartialEq, Ord, Eq)]
pub struct ValidInputFrequencies {
    sample_rate_hz: F32Frequency,
    cutoff_fr_hz: F32Frequency,
}

impl ValidInputFrequencies {
    /// Creates a new struct of validated input frequencies.
    ///
    /// `cutoff_fr_hz` is typically a value below `120 Hz` for beat detection.
    pub fn new(sample_rate_hz: f32, cutoff_fr_hz: f32) -> Result<Self, InvalidFrequencyError> {
        let sample_rate_hz = F32Frequency::try_from(sample_rate_hz)?;
        let cutoff_fr_hz = F32Frequency::try_from(cutoff_fr_hz)?;

        // Check Nyquist
        if cutoff_fr_hz.raw() * 2.0 > sample_rate_hz.raw() {
            return Err(InvalidFrequencyError::InvalidCutoffFrequency(
                sample_rate_hz,
                cutoff_fr_hz,
            ));
        }

        Ok(Self {
            sample_rate_hz,
            cutoff_fr_hz,
        })
    }

    /// Returns the sample rate (Hz).
    #[must_use]
    pub fn sample_rate_hz(&self) -> F32Frequency {
        self.sample_rate_hz
    }

    /// Returns the cutoff frequency (Hz).
    #[must_use]
    pub fn cutoff_fr_hz(&self) -> F32Frequency {
        self.cutoff_fr_hz
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio_preprocessing::audio_history::AudioHistory;
    use crate::audio_preprocessing::downsampling::{Downsampler, DownsamplingMetrics};
    use crate::audio_preprocessing::lowpass_filter::LowpassFilter;
    use crate::test_utils;
    use ringbuffer::RingBuffer;
    use std::time::Duration;
    use std::vec::Vec;

    /// Makes a few basic sanity checks with a lot of the input processing
    /// utilities in combination.
    #[test]
    fn test_lowpass_and_downsample_i16_samples() {
        let (samples, header) = test_utils::samples::sample1_single_beat();
        let sample_rate = header.sample_rate as f32;
        let duration = samples.len() as f32 * (1.0 / sample_rate);
        let original_duration = Duration::from_secs_f32(duration);
        let frequencies = ValidInputFrequencies::new(sample_rate, 120.0).unwrap();
        let mut lowpass_filter = LowpassFilter::new(frequencies);
        let metrics = DownsamplingMetrics::new(frequencies);
        let mut downsampler = Downsampler::new(metrics.clone());

        // processed samples
        let samples = lowpass_and_downsample_i16_samples_iter(
            samples.into_iter(),
            &mut lowpass_filter,
            &mut downsampler,
        )
        .collect::<Vec<_>>();

        // updated history from processed samples
        let mut history = AudioHistory::new(frequencies.sample_rate_hz, Some(metrics));
        history.update(samples.into_iter());

        let time_difference = history.passed_time() - original_duration;
        assert!(time_difference < Duration::from_millis(20));

        // now we perform some sanity checks
        let max = history
            .data()
            .iter()
            .copied()
            .map(|val| val.abs())
            .enumerate()
            .max_by(|(_, l), (_, r)| l.cmp(r))
            .unwrap();
        let info = history.index_to_sample_info(max.0);
        dbg!(info);
    }
}
