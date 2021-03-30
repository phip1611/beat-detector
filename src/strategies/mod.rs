use crate::Strategy;
use std::cell::Cell;

pub(crate) mod lpf;
pub(crate) mod spectrum;

#[derive(Debug)]
struct AnalysisState {
    sampling_rate: u32,
    // Length of each samples array ("window") that gets analyzed
    window_length: u16,
    /// Incrementing samples count. With this, window length and sampling
    /// rate, we can calculate the relative time! Start value is 0 because
    /// this struct can be created before music analysis starts. When this
    /// is 1 it means that [`crate::Strategy::is_beat`] was called the first time.
    sample_count: Cell<u32>,
}

impl AnalysisState {
    pub fn new(sampling_rate: u32, window_length: u16) -> Self {
        Self {
            sampling_rate,
            window_length,
            sample_count: Cell::new(0),
        }
    }

    #[inline(always)]
    pub fn sampling_rate(&self) -> u32 {
        self.sampling_rate
    }
    #[inline(always)]
    pub fn window_length(&self) -> u16 {
        self.window_length
    }
    #[inline(always)]
    pub fn sample_count(&self) -> u32 {
        self.sample_count.get()
    }
    #[inline(always)]
    pub fn next_beat(&self) {
        self.sample_count.replace(
            self.sample_count() + 1
        );
    }
    #[inline(always)]
    pub fn get_relative_time_ms(&self) -> u32 {
        let sampling_rate = self.sampling_rate as f32;
        // Assumes the samples has always the same length during a single run
        let samples_len = self.window_length as f32;
        let ms_per_sample = 1.0 / sampling_rate * 1000.0; // Hertz to Seconds to Milliseconds
        let ms_of_sample = ms_per_sample * samples_len;

        // minus 1.0 because "0" is the "not started yet" value and 1 the first real value
        let samples_count = self.sample_count.get() as f32 - 1.0;
        (ms_of_sample * samples_count) as u32
    }
}

