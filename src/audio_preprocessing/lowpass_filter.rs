//! Utilities for processing audio input with a lowpass filter.

use super::f32::{F32Frequency, F32Sample};
use biquad::{Biquad, Coefficients, DirectForm2Transposed, ToHertz, Type, Q_BUTTERWORTH_F32};
use crate::audio_preprocessing::ValidInputFrequencies;

/// Helper to pass samples through a lowpass filter.
///
/// Please note that using this lowpass filter introduces a short delay
/// ("group delay") onto the signal and everything is slightly shifted to
/// the right (towards now on the timescale). For a normal audio signal,
/// this can, for example, be 200 time steps at a sampling frequency of 44.1
/// kHz.
#[derive(Debug)]
pub struct LowpassFilter {
    frequencies: ValidInputFrequencies,
    /// Recommended impl of biquad filter
    filter: Option<DirectForm2Transposed<f32>>,
    /// The group delay of this filter.
    group_delay: usize,
}

impl LowpassFilter {
    /// Creates a new lowpass filter.
    pub fn new(input: ValidInputFrequencies) -> Self {
        let filter = Self::create_biquad_filter(input.sample_rate_hz(), input.cutoff_fr_hz());
        let mut this = Self {
            frequencies: input,
            filter: Some(filter),
            group_delay: 0,
        };
        this.group_delay = this.measure_group_delay();
        this
    }
    
    /// Creates a new no-op filter.
    /// 
    /// Only use this if you are sure that your input signal already ran
    /// through a lowpass filter.
    pub fn new_passthrough(input: ValidInputFrequencies) -> Self {
        Self {
            frequencies: input,
            filter: None,
            group_delay: 0,
        }
    }

    /// Runs one element through the lowpass filter and updates the internal
    /// state.
    ///
    /// Operates on [`f32`] values in range `-1.0..=1.0`: see [`F32Sample`].
    ///
    /// Please note that using this lowpass filter introduces a short delay
    /// ("group delay") onto the signal and everything is slightly shifted to
    /// the right (towards now on the timescale). For a normal audio signal,
    /// this can, for example, be 200 time steps at a sampling frequency of
    /// 44.1 kHz.
    pub fn process(&mut self, sample: F32Sample) -> F32Sample {
        if let Some(filter) = self.filter.as_mut() {
            let mut value = filter.run(sample.raw());

            if value.abs() > 1.0 {
                if value.abs().fract() < 0.1 {
                    value = value.trunc();
                }
            }

            value.try_into().unwrap()
        } else {
            sample
        }
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

    /// Measures the group delay and returns the delay in time steps (indices)
    /// to the right.
    ///
    /// The delay is expected to be constant for the entire operation. The delay
    /// is influenced by the sample rate and the cutoff frequency.
    ///
    /// The filter is reset afterwards. Therefore, this should be done once
    /// at the beginning but not during normal operation.
    fn measure_group_delay(&mut self) -> usize {
        assert!(self.filter.is_some());
        
        // I experimented a little and looked into Audacity; this seems to be 
        // a sweet spot.
        const FILTER_RESPONSE_AMPLITUDE_THRESHOLD: f32 = 0.35;
        
        let cutoff_period_s = 1.0 / self.frequencies.cutoff_fr_hz.raw();
        let cutoff_period_samples = self.frequencies.sample_rate_hz.raw() * cutoff_period_s;

        let cutoff_period_samples = cutoff_period_samples as usize;
        let remaining_samples = self.frequencies.sample_rate_hz.raw() as usize - cutoff_period_samples;

        let mut filter_response = (0 /* index */, F32Sample::try_from(0.0).unwrap() /* amplitude */);
        let mut i = 0;

        let mut apply_and_update_max_fn = |n: usize, elem: F32Sample| {
            for _ in 0..n {
                let new = self.process(elem.try_into().unwrap());

                // Exit early: We are interested in where the filter response
                // starts.
                if filter_response.1.raw() > FILTER_RESPONSE_AMPLITUDE_THRESHOLD {
                    break;
                }

                if new > filter_response.1 {
                    filter_response = (i, new);
                }
                i += 1;
            }
        };

        // We feed it with input and then wait for the response. The state is
        // updated by the function and put into `filter_response`.
        apply_and_update_max_fn(cutoff_period_samples, 1.0.try_into().unwrap());
        apply_and_update_max_fn(remaining_samples, 0.0.try_into().unwrap());

        self.filter.as_mut().unwrap().reset_state();

        filter_response.0
    }

    /// Returns the group delay of that filter.
    /// 
    /// The group delay is returned as number of samples by that the filter
    /// response is shifted "to the right" (future). TTherefore, the original
    /// index of 
    pub const fn group_delay(&self) -> usize {
        self.group_delay
    }
}

#[cfg(test)]
mod tests {
    use super::super::conversion::{f32_sample_to_i16, i16_sample_to_f32};
    use super::*;
    use crate::test_utils::target_dir_test_artifacts;
    use audio_visualizer::Channels;
    use std::vec::Vec;

    /// Creates graphical artifacts in target dir.
    #[test]
    fn test_lowpass_filter_visualize() {
        let (samples, wav) = crate::test_utils::samples::sample1_double_beat();
        let sample_rate = wav.sample_rate as f32;
        let cutoff_fr = 100.0;

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
            "double_beat__waveform_mono__default.png",
        );
        audio_visualizer::waveform::plotters_png_file::waveform_static_plotters_png_visualize(
            &lowpassed_samples,
            Channels::Mono,
            target_dir,
            "double_beat__waveform_mono__lowpassed.png",
        );
    }

    #[test]
    fn test_measure_group_delay() {
        {
            let sample_rate = 44100.0;
            let cutoff_fr = 100.0;

            let frequencies = ValidInputFrequencies::new(sample_rate, cutoff_fr).unwrap();
            let mut lowpass_filter = LowpassFilter::new(frequencies);

            // This also pretty much aligns with what we can see in Audacity.
            assert_eq!(lowpass_filter.group_delay, 310);
        }
        {

            let sample_rate = 1000.0;
            let cutoff_fr = 20.0;

            let frequencies = ValidInputFrequencies::new(sample_rate, cutoff_fr).unwrap();
            let mut lowpass_filter = LowpassFilter::new(frequencies);
            assert_eq!(lowpass_filter.group_delay, 35);
        }
    }
}
