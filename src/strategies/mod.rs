use crate::Strategy;
use std::cell::Cell;

pub(crate) mod lpf;
pub(crate) mod spectrum;

/// Structure that each [`super::Strategy`]-implementation shall use. It helps to keep
/// internal state about the ongoing analysis.
#[derive(Debug)]
struct AnalysisState {
    /// Sampling rate of the measurement. This is immutable, i.e it assumes that
    /// this value doesn't change during ongoing analysis. Value is for example
    /// 44100 Hz.
    sampling_rate: u32,
    /// Length of each samples array ("window") that gets analyzed. This is immutable,
    /// i.e it assumes that this value doesn't change during ongoing analysis.
    window_length: u16,
    /// Incrementing window count, i.e. number of sample sclices. With this, window length and sampling
    /// rate, we can calculate the relative time! Start value is 0 because
    /// this struct can be created before music analysis starts. When this
    /// is 1 it means that [`crate::Strategy::is_beat`] was called the first time.
    window_count: Cell<u32>,
    /// Timestamp of last beat
    last_beat_timestamp: Cell<u32>,
}

impl AnalysisState {

    /// Constructor for [`AnalysisState`].
    pub fn new(sampling_rate: u32, window_length: u16) -> Self {
        Self {
            sampling_rate,
            window_length,
            window_count: Cell::new(0),
            last_beat_timestamp: Cell::new(0),
        }
    }

    /// Getter for [`sampling_rate`].
    #[inline(always)]
    pub fn sampling_rate(&self) -> u32 {
        self.sampling_rate
    }

    /// Getter for [`window_length`].
    #[inline(always)]
    pub fn window_length(&self) -> u16 {
        self.window_length
    }

    /// Getter for [`window_count`].
    #[inline(always)]
    pub fn window_count(&self) -> u32 {
        self.window_count.get()
    }

    /// Increments [`window_count`] by one.
    #[inline(always)]
    pub fn inc_window_count(&self) {
        self.window_count.replace(
            self.window_count() + 1
        );
    }

    /// Getter for [`last_beat_timestamp`].
    #[inline(always)]
    pub fn last_beat_timestamp(&self) -> u32 {
        self.last_beat_timestamp.get()
    }

    /// Returns the relative time in ms since the start
    /// of the detection. It always returns the timestamp in the
    /// middle of the current window.
    #[inline(always)]
    pub fn get_relative_time_ms(&self) -> u32 {
        let sampling_rate = self.sampling_rate as f32;
        // Assumes the samples has always the same length during a single run
        let samples_len = self.window_length as f32;
        let ms_per_sample = 1.0 / sampling_rate * 1000.0; // Hertz to Seconds to Milliseconds
        let ms_per_window = ms_per_sample * samples_len;

        // minus 1.0 because "0" is the "not started yet" value and 1 the first real value
        // + 0.5 because this function returns the time in the middle of the latest window
        let window_count = self.window_count.get() as f32 - 1.0 + 0.5;
        (ms_per_window * window_count) as u32
    }

    /// Updates the timestamp of the last received beat.
    /// The time is relative to the beginning and in ms.
    #[inline(always)]
    pub fn update_last_discovered_beat_timestamp(&self) {
        self.last_beat_timestamp.replace(
            self.get_relative_time_ms()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analysis_state_get_relative_time_ms() {
        // 1/44100 * 1024 == 0.02322s == 23,22ms
        let state = AnalysisState::new(44100, 1024);
        // pretend we analyze the first window
        state.inc_window_count();
        let time = state.get_relative_time_ms();
        assert_eq!(23/2, time, "Must return timestamp in middle of first window");

        // pretend we analyze the next window
        state.inc_window_count();
        let time = state.get_relative_time_ms();
        assert_eq!((23.0 * 1.5) as u32 , time, "Must return timestamp in middle of first window");
    }
}
