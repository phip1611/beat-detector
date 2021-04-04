use lowpass_filter as lpf;
use crate::{Strategy, BeatInfo};
use crate::strategies::AnalysisState;

/// Duration in ms after each beat. Useful do prevent the same beat to be
/// detected as two beats. Value is chosen unscientifically and on will.
/// I trimmed it until the values were dense to me.
const MIN_DURATION_BETWEEN_BEATS_MS: u32 = 37;

/// Struct to provide a beat-detection strategy using a
/// lowpass filter.
#[derive(Debug)]
pub struct LpfBeatDetector {
    state: AnalysisState,
}

impl LpfBeatDetector {
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

        // BEGIN: CHECK IF NEXT BEAT WHAT BE TECHNICALLY POSSIBLE (more than x MS from last beat)
        let current_rel_time_ms = self.state.get_relative_time_ms();

        // only check this if at least a single beat was recognized
        if self.state.last_beat_timestamp() > 0 {
            let threshold = self.state.last_beat_timestamp() + MIN_DURATION_BETWEEN_BEATS_MS;
            if current_rel_time_ms < threshold {
                return None;
            }
        }

        // END

        const CUTOFF_FR: u16 = 100;
        const THRESHOLD: i16 = (i16::MAX as f32 * 0.5) as i16;
        let mut samples = samples.to_vec();
        lpf::simple::sp::apply_lpf_i16_sp(
            &mut samples,
            self.state.sampling_rate() as u16,
            CUTOFF_FR,
        );
        let is_beat = samples.iter().any(|s| s.abs() >= THRESHOLD);

        is_beat.then(|| {
            // mark we found a beat
            self.state.update_last_discovered_beat_timestamp();
            BeatInfo::new(current_rel_time_ms)
        })
    }
}

