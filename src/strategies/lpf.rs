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
use lowpass_filter as lpf;

/// Struct to provide a beat-detection strategy using a
/// lowpass filter.The algorithm is pretty basic/stupid.
/// It's not smart enough to cope with 'complex' music, like
/// most of today's pop. But it will give pretty good results
/// in 'easy' music, like most of 90s pop hits.
#[derive(Debug)]
pub struct LpfBeatDetector {
    state: AnalysisState,
}

impl LpfBeatDetector {
    #[inline(always)]
    pub fn new(sampling_rate: u32) -> Self {
        Self {
            state: AnalysisState::new(sampling_rate),
        }
    }
}

impl Strategy for LpfBeatDetector {
    /// Analyzes if inside the window of samples a beat was found after
    /// applying a lowpass filter onto the data.
    #[inline(always)]
    fn is_beat(&self, samples: &[i16]) -> Option<BeatInfo> {
        // tell the state beforehand that we are analyzing the next window - important!
        self.state.update_time(samples.len());
        // skip if distance to last beat is not fair away enough
        if !self.last_beat_beyond_threshold(&self.state) {
            return None;
        };
        // skip if the amplitude is too low, e.g. noise or silence between songs
        let w_stats = WindowStats::from(samples);
        if !self.amplitude_high_enough(&w_stats) {
            return None;
        };

        const CUTOFF_FR: u16 = 120;
        let mut samples = samples.to_vec();
        lpf::simple::sp::apply_lpf_i16_sp(
            &mut samples,
            self.state.sampling_rate() as u16,
            CUTOFF_FR,
        );
        // check if after the low pass filter we still have high amplitude
        // => then this is dominant in the window
        let threshold = (0.77 * w_stats.max() as f32) as i16;
        let is_beat = samples.iter().any(|s| s.abs() >= threshold);

        is_beat.then(|| {
            // mark we found a beat
            self.state.update_last_discovered_beat_timestamp();
            BeatInfo::new(self.state.beat_time_ms())
        })
    }

    #[inline(always)]
    fn kind(&self) -> StrategyKind {
        StrategyKind::LPF
    }

    fn name() -> &'static str
    where
        Self: Sized,
    {
        "Simple Lowpass Filter"
    }

    fn description() -> &'static str
    where
        Self: Sized,
    {
        "A simple beat detection using a lowpass filter. It's not smart enough \
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
