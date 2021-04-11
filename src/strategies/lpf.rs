use lowpass_filter as lpf;
use crate::{Strategy, BeatInfo, StrategyKind};
use crate::strategies::AnalysisState;
use crate::strategies::window_stats::WindowStats;

/// Struct to provide a beat-detection strategy using a
/// lowpass filter.
#[derive(Debug)]
pub struct LpfBeatDetector {
    state: AnalysisState,
}

impl LpfBeatDetector {

    #[inline(always)]
    pub fn new(sampling_rate: u32, window_length: u16) -> Self {
        Self {
            state: AnalysisState::new(
                sampling_rate,
                window_length
            )
        }
    }
}

impl Strategy for LpfBeatDetector {

    /// Analyzes if inside the window of samples a beat was found after
    /// applying a lowpass filter onto the data.
    #[inline(always)]
    fn is_beat(&self, samples: &[i16]) -> Option<BeatInfo> {
        if samples.len() != self.state.window_length as usize {
            panic!(
                "samples.len() == {} doesn't match the configured window_length == {}",
                samples.len(),
                self.state.window_length
            );
        }

        // tell the state beforehand that we are analyzing the next window - important!
        self.state.inc_window_count();
        // skip if distance to last beat is not fair away enough
        if self.skip_window_by_timestamp(&self.state) { return None };
        // skip if the amplitude is too low, e.g. noise or silence between songs
        let w_stats = WindowStats::from(samples);
        if self.skip_window_by_amplitude(&w_stats) { return None };

        const CUTOFF_FR: u16 = 100;
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
            BeatInfo::new(self.state.get_relative_time_ms())
        })
    }

    #[inline(always)]
    fn kind(&self) -> StrategyKind {
        StrategyKind::LPF
    }
}

