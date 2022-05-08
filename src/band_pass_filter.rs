use crate::audio_history::AudioHistoryMeta;
use crate::envelope_detector::{Envelope, EnvelopeDetector};
use crate::util::RingBufferWithSerialSliceAccess;
use biquad::{Biquad, DirectForm1, ToHertz, Type};

/// A band filter composed around [`biquad`]: one high pass filter and one low pass filter.
/// It works similar as [`biquad`]: It has an internal state and is meant
#[derive(Debug)]
pub(crate) struct BandPassFilter {
    /// Lower frequency of the band.
    lower_frequency: f32,
    /// Higher frequency of the band.
    higher_frequency: f32,
    /// Lowpass-filter from biquad.
    low_pass: DirectForm1<f32>,
    /// Highpass-filter from biquad.
    high_pass: DirectForm1<f32>,
}

impl BandPassFilter {
    /// Constructor.
    pub fn new(lower_frequency: f32, higher_frequency: f32, sampling_frequency: f32) -> Self {
        debug_assert!(lower_frequency.is_normal());
        debug_assert!(higher_frequency.is_normal());
        debug_assert!(sampling_frequency.is_normal());
        debug_assert!(
            higher_frequency <= sampling_frequency / 2.0,
            "Nyquist theorem: high frequency to high"
        );
        debug_assert!(
            lower_frequency < higher_frequency,
            "higher frequency must be higher"
        );

        let high_pass_coefficients = biquad::Coefficients::<f32>::from_params(
            Type::HighPass,
            sampling_frequency.hz(),
            lower_frequency.hz(),
            biquad::Q_BUTTERWORTH_F32,
        )
        .unwrap();
        let mut high_pass = biquad::DirectForm1::<f32>::new(high_pass_coefficients);

        let low_pass_coefficients = biquad::Coefficients::<f32>::from_params(
            Type::LowPass,
            sampling_frequency.hz(),
            higher_frequency.hz(),
            biquad::Q_BUTTERWORTH_F32,
        )
        .unwrap();
        let mut low_pass = biquad::DirectForm1::<f32>::new(low_pass_coefficients);

        Self {
            lower_frequency,
            higher_frequency,
            high_pass,
            low_pass,
        }
    }

    /// Constructor with default parameters for a low pass filter.
    pub fn new_low(sampling_rate: f32) -> Self {
        Self::new(25.0, 70.0, sampling_rate)
    }

    /// Applies a sample to the band filter and returns the bandpassed sample.
    #[inline]
    pub fn apply(&mut self, sample: f32) -> f32 {
        let high_passed_sample = self.high_pass.run(sample);
        let band_passed_sample = self.low_pass.run(high_passed_sample);
        band_passed_sample
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_band_filter() {
        // samples of a wave at (1/16)Hz with a sampling rate of 1 Hz
        let samples = [
            0.0, 0.25, 0.50, 0.75, 1.0, 0.75, 0.5, 0.25, 0.0, -0.25, -0.50, -0.75, -1.0, -0.75,
            -0.5, -0.25, 0.0,
        ];

        let signal_frequency = 1.0 / 16.0;
        let sampling_frequency = 1.0;

        // allow all frequencies
        {
            let mut band_pass_filter = BandPassFilter::new(0.000001, 0.5, sampling_frequency);

            let band_passed = samples
                .iter()
                .map(|s| band_pass_filter.apply(*s))
                .collect::<std::vec::Vec<_>>();

            for (band_passed_sample, original_sample) in band_passed.iter().zip(samples.iter()) {
                assert!(
                    (band_passed_sample - original_sample).abs() < 0.0001,
                    "band_passed_sample {band_passed_sample}, original_sample={original_sample}, difference={}",
                    (band_passed_sample - original_sample).abs()
                );
            }
        }

        // cutoff nearby signal frequency
        {
            let mut band_pass_filter =
                BandPassFilter::new(signal_frequency * 0.05, 0.5, sampling_frequency);

            let band_passed = samples
                .iter()
                .map(|s| band_pass_filter.apply(*s))
                .collect::<std::vec::Vec<_>>();

            for (band_passed_sample, original_sample) in band_passed.iter().zip(samples.iter()) {
                // 0.11 chosen at will/by testing
                assert!(
                    (band_passed_sample - original_sample).abs() < 0.11,
                    "band_passed_sample {band_passed_sample}, original_sample={original_sample}, difference={}",
                    (band_passed_sample - original_sample).abs()
                );
            }
        }

        // cutoff above signal frequency
        {
            let mut band_pass_filter = BandPassFilter::new(0.4, 0.5, sampling_frequency);

            let band_passed = samples
                .iter()
                .map(|s| band_pass_filter.apply(*s))
                .collect::<std::vec::Vec<_>>();

            for (band_passed_sample, original_sample) in band_passed.iter().zip(samples.iter()) {
                dbg!(
                    band_passed_sample,
                    original_sample,
                    (band_passed_sample - original_sample).abs()
                );
                // 0.04 chosen at will/by testing
                assert!(
                    *band_passed_sample < 0.04,
                    "band_passed_sample {band_passed_sample}, original_sample={original_sample}, difference={}",
                    (band_passed_sample - original_sample).abs()
                );
            }
        }
    }
}
