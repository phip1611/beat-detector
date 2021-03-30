use lowpass_filter as lpf;
use crate::{Strategy, BeatInfo};
use crate::strategies::AnalysisState;

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
    #[inline(always)]
    fn is_beat(&self, samples: &[i16]) -> Option<BeatInfo> {
        if samples.len() != self.state.window_length as usize {
            panic!(
                "samples.len() == {} doesn't match the configured window_length == {}",
                samples.len(),
                self.state.window_length
            );
        }

        // tell the state beforehand - important!
        self.state.next_beat();

        const CUTOFF_FR: u16 = 100;
        const THRESHOLD: i16 = (i16::MAX as f32 * 0.5) as i16;
        let mut samples = samples.to_vec();
        lpf::simple::sp::apply_lpf_i16_sp(
            &mut samples,
            self.state.sampling_rate() as u16,
            CUTOFF_FR,
        );
        let is_beat = samples.iter().any(|s| s.abs() >= THRESHOLD);

        is_beat.then(|| BeatInfo::new(
            self.state.get_relative_time_ms())
        )
    }
}

