use crate::strategies::AnalysisState;
use crate::{BeatInfo, Strategy, StrategyKind};
use spectrum_analyzer::FrequencyLimit;

/// Duration in ms after each beat. Useful do prevent the same beat to be
/// detected as two beats. Value is chosen unscientifically and on will
/// until my test worked :D
const MIN_DURATION_BETWEEN_BEATS_MS: u32 = 77;

/// Struct to provide a beat-detection strategy using a
/// Spectrum Analysis.
#[derive(Debug)]
pub struct SABeatDetector {
    state: AnalysisState,
}

impl SABeatDetector {

    #[inline(always)]
    pub fn new(sampling_rate: u32, window_length: u16) -> Self {
        // TODO window length must be a power of two
        Self {
            state: AnalysisState::new(
                sampling_rate,
                window_length
            )
        }
    }
}

impl Strategy for SABeatDetector {

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
        let samples = samples.iter().map(|x| *x as f32).collect::<Vec<_>>();

        let spectrum = spectrum_analyzer::samples_fft_to_spectrum(
            &samples,
            self.state.sampling_rate(),
            FrequencyLimit::Max(70.0),
            // scale values
            Some(&|x| x/samples.len() as f32),
            None,
        );

        if spectrum.max().1.val() > 4400.0 {
            // mark we found a beat
            self.state.update_last_discovered_beat_timestamp();
            Some(BeatInfo::new(current_rel_time_ms))
        } else {
            None
        }
    }

    #[inline(always)]
    fn kind(&self) -> StrategyKind {
        StrategyKind::Spectrum
    }
}