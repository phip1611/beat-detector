/*
MIT License

Copyright (c) 2021 Philipp Schuster

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
use crate::strategies::window_stats::WindowStats;
use crate::strategies::AnalysisState;
use crate::{BeatInfo, Strategy, StrategyKind};
use spectrum_analyzer::FrequencyLimit;

/// Struct to provide a beat-detection strategy using a
/// Spectrum Analysis. The algorithm is pretty basic/stupid.
/// It's not smart enough to cope with 'complex' music, like
/// most of today's pop. But it will give pretty good results
/// in 'easy' music, like most of 90s pop hits.
#[derive(Debug)]
pub struct SABeatDetector {
    state: AnalysisState,
}

impl SABeatDetector {
    #[inline(always)]
    pub fn new(sampling_rate: u32) -> Self {
        Self {
            state: AnalysisState::new(sampling_rate),
        }
    }

    /// Returns `frame_len` or the next power of 2.
    #[inline(always)]
    fn next_power_of_2(frame_len: usize) -> usize {
        let exponent = (frame_len as f32).log2().ceil();
        2_usize.pow(exponent as u32)
    }
}

impl Strategy for SABeatDetector {
    /// Analyzes if inside the window of samples a beat was found after
    /// applying a lowpass filter onto the data.
    #[inline(always)]
    fn is_beat(&self, orig_samples: &[i16]) -> Option<BeatInfo> {
        // make sure buffer has length that is a power of two for FFT
        let len_power_of_2 = Self::next_power_of_2(orig_samples.len());
        let diff = len_power_of_2 - orig_samples.len();
        let mut samples = Vec::with_capacity(len_power_of_2);
        samples.extend_from_slice(orig_samples);
        samples.extend_from_slice(&vec![0; diff]);

        // tell the state beforehand that we are analyzing the next window - important!
        self.state.update_time(samples.len());
        // skip if distance to last beat is not fair away enough
        if !self.last_beat_beyond_threshold(&self.state) {
            return None;
        };
        // skip if the amplitude is too low, e.g. noise or silence between songs
        let w_stats = WindowStats::from(samples.as_slice());
        if !self.amplitude_high_enough(&w_stats) {
            return None;
        };

        let samples = samples.iter().map(|x| *x as f32).collect::<Vec<_>>();

        let spectrum = spectrum_analyzer::samples_fft_to_spectrum(
            &samples,
            self.state.sampling_rate(),
            FrequencyLimit::Max(90.0),
            // scale values
            Some(&|x| x / samples.len() as f32),
            None,
        );

        if spectrum.max().1.val() > 4400.0 {
            // mark we found a beat
            self.state.update_last_discovered_beat_timestamp();
            Some(BeatInfo::new(self.state.beat_time_ms()))
        } else {
            None
        }
    }

    #[inline(always)]
    fn kind(&self) -> StrategyKind {
        StrategyKind::Spectrum
    }

    fn name() -> &'static str
    where
        Self: Sized,
    {
        "Simple Spectrum Analysis"
    }

    fn description() -> &'static str
    where
        Self: Sized,
    {
        "A simple beat detection using a spectrum analysis. It's not smart enough \
        to cope with 'complex' music, like most of today's pop. But it will give \
        pretty good results in 'easy' music, like most of 90s pop hits."
    }

    /// Value chosen at will. It is really high because this strategy
    /// is stupid. It's not smart enough to detect a "slowly decreasing beat",
    /// i.e. it may detect the same beat twice otherwise.
    #[inline(always)]
    fn min_duration_between_beats_ms() -> u32
    where
        Self: Sized,
    {
        400
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_next_power_of_2() {
        assert_eq!(2, SABeatDetector::next_power_of_2(2));
        assert_eq!(16, SABeatDetector::next_power_of_2(16));
        assert_eq!(128, SABeatDetector::next_power_of_2(127));
        assert_eq!(128, SABeatDetector::next_power_of_2(128));
        assert_eq!(256, SABeatDetector::next_power_of_2(129));
    }
}
