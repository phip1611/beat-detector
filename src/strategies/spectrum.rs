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
use ringbuffer::{ConstGenericRingBuffer, RingBufferWrite, RingBufferExt};
use std::cell::RefCell;
use spectrum_analyzer::scaling::divide_by_N;

/// Struct to provide a beat-detection strategy using a
/// Spectrum Analysis. The algorithm is pretty basic/stupid.
/// It's not smart enough to cope with 'complex' music, like
/// most of today's pop. But it will give pretty good results
/// in 'easy' music, like most of 90s pop hits.
#[derive(Debug)]
pub struct SABeatDetector {
    state: AnalysisState,
    // ring buffer with latest audio; necessary because we don't
    // necessarily get 1024 samples at each callback but mostly
    // 500-540..
    audio_data_buf: RefCell<ConstGenericRingBuffer<f32, 1024>>,
}

impl SABeatDetector {
    #[inline(always)]
    pub fn new(sampling_rate: u32) -> Self {
        const LEN: usize = 1024;
        let mut initial_buf = ConstGenericRingBuffer::<f32, LEN>::new();
        (0..LEN).for_each(|_| initial_buf.push(0.0));
        Self {
            state: AnalysisState::new(sampling_rate),
            audio_data_buf: RefCell::from(initial_buf),
        }
    }
}

impl Strategy for SABeatDetector {
    /// Callback called when the audio input library got the next callback.
    #[inline(always)]
    fn is_beat(&self, callback_samples: &[i16]) -> Option<BeatInfo> {
        // make sure we have the latest 1024 audio samples in the buffer
        // => ready for FFT
        let mut audio_data_buf = self.audio_data_buf.borrow_mut();
        for sample in callback_samples {
            audio_data_buf.push(*sample as f32);
        }

        // tell the state beforehand that we are analyzing the next window - important!
        self.state.update_time(callback_samples.len());
        // skip if distance to last beat is not far away enough
        if !self.last_beat_beyond_threshold(&self.state) {
            return None;
        };
        // skip if the amplitude is too low, e.g. noise or silence between songs
        let w_stats = WindowStats::from(callback_samples);
        if !self.amplitude_high_enough(&w_stats) {
            return None;
        };

        let spectrum = spectrum_analyzer::samples_fft_to_spectrum(
            &audio_data_buf.to_vec(),
            self.state.sampling_rate(),
            FrequencyLimit::Max(90.0),
            // None,
            Some(&divide_by_N),
        ).unwrap();

        // I don't know what the value really means :D
        // figured out by testing.. :/
        if spectrum.max().1.val() > 2_100_000.0 {
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

    // use super::*;

}