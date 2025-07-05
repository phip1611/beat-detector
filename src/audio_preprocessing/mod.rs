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

    downsampler.downsample_iter(iter)
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
    use crate::audio_analysis::max_min_iterator::MaxMinIterator;
    use crate::audio_preprocessing::audio_history::AudioHistory;
    use crate::audio_preprocessing::downsampling::{Downsampler, DownsamplingMetrics};
    use crate::audio_preprocessing::lowpass_filter::LowpassFilter;
    use crate::test_utils;
    use crate::test_utils::target_dir_test_artifacts;
    use audio_visualizer::Channels;
    use hound::{SampleFormat, WavSpec};
    use ringbuffer::RingBuffer;
    use std::time::Duration;
    use std::vec::Vec;

    fn process_samples(
        frequencies: ValidInputFrequencies,
        samples: &[i16],
        do_lowpass: bool,
        do_downsample: bool,
    ) -> (AudioHistory, Vec<i16>) {
        let metrics = if do_downsample {
            DownsamplingMetrics::new(frequencies.clone())
        } else {
            DownsamplingMetrics::new_disabled(frequencies.clone())
        };
        let mut downsampler = Downsampler::new(metrics.clone());

        let mut lowpass_filter = if do_lowpass {
            LowpassFilter::new(frequencies)
        } else {
            LowpassFilter::new_passthrough(frequencies)
        };

        // processed samples
        let samples = lowpass_and_downsample_i16_samples_iter(
            samples.iter().copied(),
            &mut lowpass_filter,
            &mut downsampler,
        )
        .collect::<Vec<_>>();

        let mut history = AudioHistory::new(
            frequencies.sample_rate_hz,
            Some(metrics),
            Some(lowpass_filter.group_delay()),
        );
        history.update(samples.iter().copied());

        (history, samples)
    }

    fn write_wav_file(name: &str, samples: &[i16], sample_rate: u32) {
        let mut wav_path = target_dir_test_artifacts();
        wav_path.push(name);

        let mut wav_writer = hound::WavWriter::create(
            wav_path,
            WavSpec {
                channels: 1,
                sample_rate,
                bits_per_sample: 16,
                sample_format: SampleFormat::Int,
            },
        )
        .unwrap();

        for &sample in samples {
            wav_writer.write_sample(sample).unwrap()
        }
        wav_writer.finalize().unwrap();
    }

    /// Makes a few basic sanity checks with a lot of the input processing
    /// utilities in combination.
    #[test]
    fn test_lowpass_downsample_integration_in_combination() {
        //let (samples_orig, header) = test_utils::samples::sample1_double_beat();
        let (samples_orig, header) = test_utils::samples::holiday_long();
        let sample_rate = header.sample_rate as f32;
        let duration = samples_orig.len() as f32 * (1.0 / sample_rate);
        let original_duration = Duration::from_secs_f32(duration);
        let frequencies = ValidInputFrequencies::new(sample_rate, 200.0).unwrap();

        let (history_untouched, samples_untouched) =
            process_samples(frequencies, &samples_orig, false, false);
        let (history_downsampled, samples_downsampled) =
            process_samples(frequencies, &samples_orig, false, true);
        let (history_lowpassed, samples_lowpassed) =
            process_samples(frequencies, &samples_orig, true, false);
        let (history_lowpassed_downsampled, samples_lowpassed_downsampled) =
            process_samples(frequencies, &samples_orig, true, true);

        // time checks: minor differences are okay because of f32 multiplication
        {
            let drift = history_untouched.passed_time().abs_diff(original_duration);
            assert!(drift < Duration::from_millis(2));
        }
        {
            let drift = history_downsampled
                .passed_time()
                .abs_diff(original_duration);
            assert!(drift < Duration::from_millis(2));
        }
        {
            let drift = history_lowpassed.passed_time().abs_diff(original_duration);
            assert!(drift < Duration::from_millis(2));
        }
        {
            let drift = history_lowpassed_downsampled
                .passed_time()
                .abs_diff(original_duration);
            assert!(drift < Duration::from_millis(2));
        }

        {
            write_wav_file(
                "sample1_double_beat__untouched.wav",
                &samples_orig,
                history_untouched.sample_rate().raw() as u32,
            );
            write_wav_file(
                "sample1_double_beat__lowpassed.wav",
                &samples_lowpassed,
                history_lowpassed.sample_rate().raw() as u32,
            );
            write_wav_file(
                "sample1_double_beat__downsampled.wav",
                &samples_downsampled,
                history_downsampled.sample_rate().raw() as u32,
            );
            write_wav_file(
                "sample1_double_beat__lowpassed_downsampled.wav",
                &samples_lowpassed_downsampled,
                history_lowpassed_downsampled.sample_rate().raw() as u32,
            );
        }

        let peaks_untouched = MaxMinIterator::new(&history_untouched, None).collect::<Vec<_>>();
        let peaks_downsampled = MaxMinIterator::new(&history_downsampled, None).collect::<Vec<_>>();
        let peaks_lowpassed = MaxMinIterator::new(&history_lowpassed, None).collect::<Vec<_>>();
        let peaks_lowpassed_downsampled =
            MaxMinIterator::new(&history_lowpassed_downsampled, None).collect::<Vec<_>>();

        {
            let target_dir = target_dir_test_artifacts();
            let target_dir = target_dir.as_os_str().to_str().unwrap();

            let samples = history_untouched.data().iter().copied().collect::<Vec<_>>();
            audio_visualizer::waveform::plotters_png_file::waveform_static_plotters_png_visualize(
                &samples,
                Channels::Mono,
                target_dir,
                "normal.png",
            );

            let samples = history_lowpassed.data().iter().copied().collect::<Vec<_>>();
            audio_visualizer::waveform::plotters_png_file::waveform_static_plotters_png_visualize(
                &samples,
                Channels::Mono,
                target_dir,
                "lowpassed.png",
            );
        }

        // One can also verify this in Audacity.
        assert_eq!(peaks_untouched.len(), 52);
        assert_eq!(peaks_lowpassed.len(), 52);
        assert_eq!(peaks_downsampled.len(), 52);
        dbg!(&peaks_downsampled);
        assert_eq!(peaks_lowpassed_downsampled.len(), 52);
    }
}
