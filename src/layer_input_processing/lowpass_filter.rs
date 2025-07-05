//! Utilities for processing audio input with a lowpass filter.

use super::f32::{F32Frequency, F32Sample};
use crate::ValidInputFrequencies;
use biquad::{Biquad, Coefficients, DirectForm2Transposed, ToHertz, Type, Q_BUTTERWORTH_F32};

/// Helper to pass samples through a lowpass filter.
#[derive(Debug)]
pub struct LowpassFilter {
    frequencies: ValidInputFrequencies,
    // Recommended impl of biquad filter
    filter: DirectForm2Transposed<f32>,
}

impl LowpassFilter {
    /// Creates a new lowpass filter.
    ///
    /// The sample rate must be a multiple of the cutoff frequency.
    pub fn new(input: ValidInputFrequencies) -> Self {
        let filter = Self::create_biquad_filter(input.sample_rate_hz(), input.cutoff_fr_hz());
        Self {
            frequencies: input,
            filter,
        }
    }

    /// Runs one element through the lowpass filter and updates the internal
    /// state.
    ///
    /// Operates on [`f32`] values in range `-1.0..=1.0`: see [`F32Sample`].
    pub fn process(&mut self, sample: F32Sample) -> F32Sample {
        let value = self.filter.run(sample.raw());
        value.try_into().unwrap()
    }

    /// Creates a properly configured [`biquad`] filter acting as lowpass filter.
    fn create_biquad_filter(
        sample_rate_hz: F32Frequency,
        cutoff_fr_hz: F32Frequency,
    ) -> DirectForm2Transposed<f32> {
        let f0 = cutoff_fr_hz.raw().hz();
        let fs = sample_rate_hz.raw().hz();

        let coefficients =
            Coefficients::<f32>::from_params(Type::LowPass, fs, f0, Q_BUTTERWORTH_F32).unwrap();
        DirectForm2Transposed::<f32>::new(coefficients)
    }
}

#[cfg(test)]
mod tests {
    use super::super::conversion::{f32_sample_to_i16, i16_sample_to_f32};
    use super::*;
    use crate::test_utils::target_dir_test_artifacts;
    use audio_visualizer::Channels;
    use std::vec::Vec;

    /// Creates artifacts in target/
    #[test]
    fn test_lowpass_filter_visualize() {
        let (samples, wav) = crate::test_utils::samples::sample1_long();
        let sample_rate = wav.sample_rate as f32;
        let cutoff_fr = 100.0;
        eprintln!("sample_rate={sample_rate} Hz, cutoff_fr={cutoff_fr} Hz");

        let frequencies = ValidInputFrequencies::new(sample_rate, cutoff_fr).unwrap();
        let mut lowpass_filter = LowpassFilter::new(frequencies);

        let lowpassed_samples = samples
            .iter()
            .copied()
            .map(i16_sample_to_f32)
            .map(|sample| lowpass_filter.process(sample))
            .map(|sample| f32_sample_to_i16(sample.raw()).unwrap())
            .collect::<Vec<_>>();

        let target_dir = target_dir_test_artifacts();
        let target_dir = target_dir.as_os_str().to_str().unwrap();

        audio_visualizer::waveform::plotters_png_file::waveform_static_plotters_png_visualize(
            &samples,
            Channels::Mono,
            target_dir,
            "holiday_long_waveform_mono_default.png",
        );
        audio_visualizer::waveform::plotters_png_file::waveform_static_plotters_png_visualize(
            &lowpassed_samples,
            Channels::Mono,
            target_dir,
            "holiday_long_waveform_mono_lowpassed.png",
        );
    }
}
