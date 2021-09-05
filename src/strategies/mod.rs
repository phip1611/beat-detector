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
use std::cell::Cell;

pub(crate) mod lpf;
pub(crate) mod spectrum;
pub mod window_stats;

/// Structure that each [`super::Strategy`]-implementation shall use. It helps to keep
/// internal state about the ongoing analysis, i.e. the progress in time. It is
/// capable to work with different window/frame sizes. This is especially required
/// for Linux because right now it doesn't work there to use a fixed buffer size -_-
///
/// This struct shall be updated live/on the fly while music is recorded. Therefore,
/// the time inside this struct is almost the real (relative) time from the beginning
/// of recording, despite some latency.
#[derive(Debug)]
pub struct AnalysisState {
    /// Sampling rate of the measurement. This is immutable, i.e it assumes that
    /// this value doesn't change during ongoing analysis. Value is for example
    /// 44100 Hz.
    sampling_rate: u32,
    /// Calculated once by `1 / ms_per_sample`. Done once to speed up calculation.
    ms_per_sample: f32,
    /// This is always a bit shorter than [`time_ms`]. It is the timestamp
    /// in the middle of the current frame, whereas [`time_ms`] is the timestamp
    /// at the end. This time will be attached to a beat if one was found in the
    /// current frame/window.
    beat_time_ms: Cell<u32>,
    /// The ongoing  relative progress in time in ms of the recording.
    time_ms: Cell<u32>,
    /// Timestamp of last beat. This is always a value that was previously in
    /// [`beat_time_ms`].
    last_beat_timestamp: Cell<u32>,
}

impl AnalysisState {
    /// Constructor for [`AnalysisState`].
    pub fn new(sampling_rate: u32) -> Self {
        Self {
            sampling_rate,
            // Hertz => second => milli seconds
            ms_per_sample: 1.0 / sampling_rate as f32 * 1000.0,
            beat_time_ms: Cell::new(0),
            time_ms: Cell::new(0),
            last_beat_timestamp: Cell::new(0),
        }
    }

    /// Updates the total passed internal time. It does so by calculating the milliseconds of
    /// the amount of (mono, not stereo!) samples for the given [`sampling_rate`].
    /// It always adds the timestamp in the middle of the current window/frame to
    /// the current value.
    #[inline(always)]
    pub fn update_time(&self, frame_len: usize) {
        // if 44,1kHz is sampling rate and we have 44,1k samples => 1s
        let ms_of_frame = self.ms_per_sample * frame_len as f32;
        self.beat_time_ms.set(
            // beat time is in the half of the window/frame
            self.time_ms.get() + (ms_of_frame / 2.0) as u32,
        );
        self.time_ms.set(self.time_ms.get() + ms_of_frame as u32);
    }

    /// Updates the timestamp of the last received beat.
    /// The time is relative to the beginning and in ms.
    #[inline(always)]
    pub fn update_last_discovered_beat_timestamp(&self) {
        self.last_beat_timestamp.replace(self.beat_time_ms.get());
    }

    /// Getter for [`sampling_rate`].
    #[inline(always)]
    pub const fn sampling_rate(&self) -> u32 {
        self.sampling_rate
    }

    /// Getter for [`last_beat_timestamp`].
    #[inline(always)]
    pub fn last_beat_timestamp(&self) -> u32 {
        self.last_beat_timestamp.get()
    }

    /// Getter for [`beat_time_ms`].
    #[inline(always)]
    pub fn beat_time_ms(&self) -> u32 {
        self.beat_time_ms.get()
    }

    /// Getter for [`time_ms`].
    #[inline(always)]
    pub fn time_ms(&self) -> u32 {
        self.time_ms.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analysis_state_get_relative_time_ms() {
        // 1/44100 * 1024 == 0.02322s == 23,22ms per 1024 frames
        let state = AnalysisState::new(44100);
        // pretend we analyze the first window of 1024 samples
        state.update_time(1024);
        assert_eq!(
            23 / 2,
            state.beat_time_ms(),
            "Must return timestamp in middle of first window"
        );
        assert_eq!(
            23,
            state.time_ms(),
            "Must return timestamp at end of first window"
        );

        // pretend we analyze the next window of 1024 samples
        state.update_time(1024);
        assert_eq!(
            (23.0 * 1.5) as u32,
            state.beat_time_ms(),
            "Must return timestamp in middle of second window"
        );
        assert_eq!(
            46,
            state.time_ms(),
            "Must return timestamp at end of second window"
        );

        // pretend we analyze the next window of only 317 samples
        state.update_time(317);
        assert_eq!(
            (46 + (317.0 / 2.0 * 1.0 / 44100.0 * 1000.0) as u32),
            state.beat_time_ms(),
            "Must return timestamp in middle of third window"
        );
        assert_eq!(
            (46 + (317.0 / 1.0 / 44100.0 * 1000.0) as u32),
            state.time_ms(),
            "Must return timestamp at end of third window"
        );
    }
}
