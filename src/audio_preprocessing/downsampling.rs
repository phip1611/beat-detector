//! Utilities for downsampling audio input.
//!
//! Downsampling helps to massively reduce the cost of each analysis operation
//! in the end. Due to the nature of the lowpass filter, we can work with much
//! less data.

use super::f32::F32Frequency;
use crate::audio_preprocessing::ValidInputFrequencies;

/// Bundles the downsampling metrics.
#[derive(Debug, Clone, PartialEq)]
pub struct DownsamplingMetrics {
    /// The downsampling factor.
    factor: usize,
    /// New effective sample rate.
    effective_sample_rate_hz: F32Frequency,
    /// Original sample rate and cutoff frequency.
    input: ValidInputFrequencies,
}

impl DownsamplingMetrics {
    /// Creates a new struct and calculates the optimum downsampling factor.
    pub fn new(input: ValidInputFrequencies) -> Self {
        let (factor, effective_sample_rate_hz) = Self::calculate_max_downsampling_factor(&input);
        Self {
            factor,
            effective_sample_rate_hz,
            input,
        }
    }

    /// Calculates the metrics for a maximum effective downsampling with the
    /// given [`ValidInputFrequencies`].
    ///
    /// The high-level goal of that factor is to:
    /// - reduce the number of samples (after lowpass-filtering them) for the
    ///   analysis to a minimum => smaller memory footprint, faster calculation
    /// - respect the Nyquist frequency for the given cutoff frequency
    ///
    /// The factor means: how many samples to skip, respectively, only to take
    /// every `n`th (factor) sample into account and ignore the rest.
    ///
    /// We search for the largest possible integer divisor of `sample_rate` such
    /// that the result:
    /// - is still ≥ 2 * cutoff,
    /// - and gives exact sampling rate (no fractional part)
    fn calculate_max_downsampling_factor(
        input: &ValidInputFrequencies,
    ) -> (usize /* factor */, F32Frequency) {
        // Nyquist: sample rate >= 2 * min frequency
        let nyquist_fr_hz = input.cutoff_fr_hz.raw() * 2.0;

        // The biquad filter is not perfect. To prevent aliasing and other weird
        // effects, we artificially reduce the theoretically minimum possible
        // factor.
        let min_fr_safe = nyquist_fr_hz * 2.0;

        let min_factor = 1;
        let max_factor = input.sample_rate_hz.raw() as u32 / 2;

        // Variables that will be overwritten by the loop.
        let mut factor = min_factor;
        let mut effective_sample_rate_hz = input.sample_rate_hz;

        // We iterate all possible factors. This is much more efficient than
        // to iterate possible frequencies. We want to find the highest factor.
        // From that factor, we can also calculate the new effective sampling
        // rate.
        for maybe_factor in (min_factor..=max_factor).rev() {
            let dividend = input.sample_rate_hz.raw() as u32;
            let (quotient, remainder) = (dividend / maybe_factor, dividend % maybe_factor);
            if remainder != 0 {
                continue;
            }

            let maybe_effective_sample_rate_hz = quotient;
            if maybe_effective_sample_rate_hz < min_fr_safe as u32 {
                continue;
            }

            factor = maybe_factor;
            effective_sample_rate_hz =
                F32Frequency::try_from(maybe_effective_sample_rate_hz as f32).unwrap();

            break;
        }

        (factor as usize, effective_sample_rate_hz)
    }

    /// Returns the downsampling factor.
    pub fn factor(&self) -> usize {
        self.factor
    }

    /// Returns the new effective sample rate.
    pub fn effective_sample_rate_hz(&self) -> F32Frequency {
        self.effective_sample_rate_hz
    }
}

#[derive(Debug)]
pub struct DownsamplerIter<'a, T, I: Iterator<Item = T>> {
    samples_iter: I,
    downsampler: &'a mut Downsampler,
}

impl<'a, T, I: Iterator<Item = T>> Iterator for DownsamplerIter<'a, T, I> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        // We iterate the inner iterator and emit the next element if it
        // fulfills the criteria.
        while let Some(sample) = self.samples_iter.next() {
            let should_emit = self.downsampler.i % self.downsampler.n_th == 0;

            // Increment but truncate to 0 if bound is exceeded.
            self.downsampler.i += 1;
            if self.downsampler.i == self.downsampler.n_th {
                self.downsampler.i = 0;
            }

            if should_emit {
                return Some(sample);
            }
        }
        None
    }
}

/// Helper to downsample audio samples from a higher sample rate to a lower
/// sample rate.
///
/// **Caution!** To prevent aliasing effects, the audio source needs to to be
#[derive(Clone, Debug)]
pub struct Downsampler {
    /// The current element counter. Everytime this is `0`, the next element
    /// should be emitted, and this value incremented. Once this value is
    /// `>= n_th`, it should be truncated back to `0`.
    // mutated during operation
    i: usize,
    /// Indicates which `n`th element from input collections we are interested
    /// in.
    // fixed during operation
    n_th: usize,
    metrics: DownsamplingMetrics,
}

impl Downsampler {
    /// Creates a new downsampler for the given frequencies.
    ///
    /// The sample rate must be a multiple of the cutoff frequency.
    #[must_use]
    pub fn new(metrics: DownsamplingMetrics) -> Self {
        let n_th = metrics.factor;
        Self {
            i: 0,
            n_th,
            metrics,
        }
    }

    /// Performs a downsample operation on the given samples.
    ///
    /// The returned iterator must be consumed.
    #[must_use]
    pub fn downsample<T, In: IntoIterator<Item = T, IntoIter = I>, I: Iterator<Item = T>>(
        &mut self,
        samples_iter: In,
    ) -> DownsamplerIter<T, I> {
        let samples_iter = samples_iter.into_iter();
        DownsamplerIter {
            samples_iter,
            downsampler: self,
        }
    }

    /// Returns the underlying metrics.
    pub fn metrics(&self) -> &DownsamplingMetrics {
        &self.metrics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    #[test]
    fn test_calculate_downsampling_metrics() {
        {
            let input = ValidInputFrequencies::new(1000.0, 100.0).unwrap();
            let metrics = DownsamplingMetrics::new(input);
            assert_eq!(metrics.factor, 2);
            assert_eq!(metrics.effective_sample_rate_hz.raw(), 500.0);
        }
        {
            let input = ValidInputFrequencies::new(1000.0, 250.0).unwrap();
            let metrics = DownsamplingMetrics::new(input);
            // no reduction at all, not safe
            assert_eq!(metrics.factor, 1);
            assert_eq!(metrics.effective_sample_rate_hz.raw(), 1000.0);
        }
        {
            let input = ValidInputFrequencies::new(44100.0, 100.0).unwrap();
            let metrics = DownsamplingMetrics::new(input);
            assert_eq!(metrics.factor, 105);
            assert_eq!(metrics.effective_sample_rate_hz.raw(), 420.0);
        }
        {
            let input = ValidInputFrequencies::new(44100.0, 600.0).unwrap();
            let metrics = DownsamplingMetrics::new(input);
            assert_eq!(metrics.factor, 18);
            assert_eq!(metrics.effective_sample_rate_hz.raw(), 2450.0);
        }
        {
            let input = ValidInputFrequencies::new(44100.0, 10.0).unwrap();
            let metrics = DownsamplingMetrics::new(input);
            assert_eq!(metrics.factor, 1050);
            assert_eq!(metrics.effective_sample_rate_hz.raw(), 42.0);
        }
        {
            let input = ValidInputFrequencies::new(44100.0, 17.0).unwrap();
            let metrics = DownsamplingMetrics::new(input);
            assert_eq!(metrics.factor, 630);
            assert_eq!(metrics.effective_sample_rate_hz.raw(), 70.0);
        }
    }

    #[test]
    fn test_downsampler_update_internal_index_correctly() {
        let input = ValidInputFrequencies::new(36.0, 3.0).unwrap();
        let metrics = DownsamplingMetrics::new(input);
        assert_eq!(metrics.factor, 3);

        // Downsampler that emits every 3rd item.
        let mut downsampler = Downsampler::new(metrics);
        assert_eq!(downsampler.i, 0);

        let _ = downsampler.downsample([0_i16]).count();
        assert_eq!(downsampler.i, 1);

        let _ = downsampler.downsample([0_i16]).count();
        assert_eq!(downsampler.i, 2);

        let _ = downsampler.downsample([0_i16]).count();
        assert_eq!(downsampler.i, 0);

        let _ = downsampler.downsample([0_i16]).count();
        assert_eq!(downsampler.i, 1);

        let _ = downsampler.downsample([0_i16]).count();
        assert_eq!(downsampler.i, 2);
    }

    #[test]
    fn test_downsampler_i16() {
        let input = ValidInputFrequencies::new(36.0, 3.0).unwrap();
        let metrics = DownsamplingMetrics::new(input);
        assert_eq!(metrics.factor, 3);

        // Downsampler that emits every 3rd item.
        let mut downsampler = Downsampler::new(metrics);

        assert_eq!(downsampler.i, 0);

        let down_sampled = downsampler
            .downsample([0_i16, 1, 2, 3, 4, 5, 6, 7])
            .collect::<Vec<_>>();
        assert_eq!(down_sampled, vec![0, 3, 6]);

        let down_sampled = downsampler.downsample([8, 9, 10, 11]).collect::<Vec<_>>();
        assert_eq!(down_sampled, vec![9]);

        let down_sampled = downsampler.downsample([12, 13]).collect::<Vec<_>>();
        assert_eq!(down_sampled, vec![12]);

        assert_eq!(downsampler.i, 2);
    }

    #[test]
    fn test_downsampler_f32() {
        let input = ValidInputFrequencies::new(36.0, 3.0).unwrap();
        let metrics = DownsamplingMetrics::new(input);
        assert_eq!(metrics.factor, 3);

        // Downsampler that emits every 3rd item.
        let mut downsampler = Downsampler::new(metrics);

        let down_sampled = downsampler
            .downsample([0.0_f32, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0])
            .collect::<Vec<_>>();
        assert_eq!(down_sampled, vec![0.0, 3.0, 6.0]);

        let down_sampled = downsampler
            .downsample([8.0, 9.0, 10.0, 11.0])
            .collect::<Vec<_>>();
        assert_eq!(down_sampled, vec![9.0]);

        let down_sampled = downsampler.downsample([12.0, 13.0]).collect::<Vec<_>>();
        assert_eq!(down_sampled, vec![12.0]);

        assert_eq!(downsampler.i, 2);
    }
}
