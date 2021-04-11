use crate::strategies::AnalysisState;
use crate::strategies::lpf::LpfBeatDetector;
use crate::strategies::spectrum::SABeatDetector;
use crate::strategies::window_stats::WindowStats;

mod strategies;
pub mod record;

/// Struct that holds information about a detected beat.
#[derive(Debug)]
pub struct BeatInfo {
    relative_ms: u32,
    // todo intensity
}
impl BeatInfo {

    #[inline(always)]
    pub fn new(relative_ms: u32) -> Self {
        Self {
            relative_ms,
        }
    }

    #[inline(always)]
    pub fn relative_ms(&self) -> u32 {
        self.relative_ms
    }
}

/// Duration in ms after each beat. Useful do prevent the same beat to be
/// detected as two beats. Value is chosen unscientifically and on will
/// until my test worked :D
const MIN_DURATION_BETWEEN_BEATS_MS: u32 = 150;

/// Common abstraction over a beat detection strategy. Each strategy keeps ongoing
/// audio samples, for example from microphone. Strategies should have an internal
/// mutable state via interior mutability to compare sample windows (and analysis)
/// against previous values.
pub trait Strategy {

    /// Checks if inside the samples window a new beat was recognized.
    /// If so, it returns `Some` with [`BeatInfo`] as payload.
    ///
    /// Implementations may buffer previous samples and combine them with the latest,
    /// i.e. make a sliding window.
    fn is_beat(&self, samples: &[i16]) -> Option<BeatInfo>;

    /// Convenient getter to get the [`StrategyKind`] of a strategy.
    /// This is a 1:1 mapping.
    fn kind(&self) -> StrategyKind;

    /// Abstract implementation for all strategies which checks if
    /// the current window should be skipped, because the distance
    /// to the last beat is to short.
    #[inline(always)]
    fn skip_window_by_timestamp(&self, state: &AnalysisState) -> bool {
        // BEGIN: CHECK IF NEXT BEAT WHAT BE TECHNICALLY POSSIBLE (more than x MS from last beat)
        let current_rel_time_ms = state.get_relative_time_ms();

        // only check this if at least a single beat was recognized
        if state.last_beat_timestamp() > 0 {
            let threshold = state.last_beat_timestamp() + MIN_DURATION_BETWEEN_BEATS_MS;
            if current_rel_time_ms < threshold {
                return true;
            }
        }
        false
    }

    /// Abstract implementation for all strategies which checks if
    /// the current windows max amplitude is below a value where
    /// a beat could happen in theory (e.g. just noise between songs)
    #[inline(always)]
    fn skip_window_by_amplitude(&self, w_stats: &WindowStats) -> bool {
        const MIN_AMPLITUDE_THRESHOLD: i16 = (i16::MAX as f32 * 0.3) as i16;
        w_stats.max() <= MIN_AMPLITUDE_THRESHOLD as u16
    }
}

/// Enum that conveniently and easily makes all [`Strategy`]s provided by this crate accessible.
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum StrategyKind {
    /// Lowpass-Filter. Corresponds to [`strategies::lpf::LpfBeatDetector`].
    LPF,
    /// Frequency analysis. Corresponds to TODO.
    Spectrum,
}

impl StrategyKind {

    /// Creates a concrete detector object, i.e. a struct that implements
    /// [`Strategy`] on that you can continuously analyze your input audio data.
    /// Supp
    #[inline(always)]
    fn detector(&self, sampling_rate: u32, window_length: u16) -> Box<dyn Strategy + Send> {
        match self {
            StrategyKind::LPF => Box::new(LpfBeatDetector::new(sampling_rate, window_length)),
            StrategyKind::Spectrum => Box::new(SABeatDetector::new(sampling_rate, window_length)),
            _ => panic!("Unknown Strategy")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use minimp3::{Decoder as Mp3Decoder, Error as Mp3Error, Frame as Mp3Frame};
    use std::fs::File;
    use std::collections::HashMap;

    // opened the file in Audacity and looked where the
    // beats are
    const SAMPLE_1_EXPECTED_BEATS_MS: [u32; 6] = [
        300,
        2131,
        2297,
        4303,
        6143,
        6310,
    ];

    #[test]
    fn test_sample_1_print_beats() {
        let (sample_1_audio_data, sampling_rate) = read_mp3_to_mono("res/sample_1.mp3");
        // assert 44,1kHz because it makes things easier
        assert_eq!(sampling_rate, 44100, "The sampling rate of the MP3 examples must be 44100Hz.");

        // 1/44100 * 1024 == 1024/44100 == 0.046439s == 23,2ms
        let window_length = 1024;

        let map = apply_samples_to_all_strategies(
            window_length,
            &sample_1_audio_data,
            sampling_rate
        );

        for (strategy, beats) in map {
            println!("Strategy {:?} found beats at:", strategy);
            for beat in beats {
                println!("  {}ms", beat.relative_ms());
            }
        }
    }

    #[test]
    fn test_sample_1_beat_detection() {
        let (sample_1_audio_data, sampling_rate) = read_mp3_to_mono("res/sample_1.mp3");
        // assert 44,1kHz because it makes things easier
        assert_eq!(sampling_rate, 44100, "The sampling rate of the MP3 examples must be 44100Hz.");

        // 1/44100 * 1024 == 1024/44100 == 0.046439s == 23,2ms
        let window_length = 1024;

        let map = apply_samples_to_all_strategies(
            window_length,
            &sample_1_audio_data,
            sampling_rate
        );

        const DIFF_WARN_MS: u32 = 30;
        const DIFF_ERROR_MS: u32 = 60;

        for (strategy, beats) in map {
            assert_eq!(SAMPLE_1_EXPECTED_BEATS_MS.len(), beats.len(), "Strategy {:?} must detect {} beats in sample 1!", strategy, SAMPLE_1_EXPECTED_BEATS_MS.len());
            for (i, beat) in beats.iter().enumerate() {
                let abs_diff = (SAMPLE_1_EXPECTED_BEATS_MS[i] as i64 - beat.relative_ms() as i64).abs() as u32;
                assert!(abs_diff < DIFF_ERROR_MS, "[{:?}]: Recognized beat[{}] should not be more than {} ms away from the actual value; is {}ms", strategy, i, DIFF_ERROR_MS, abs_diff);
                if abs_diff >= DIFF_WARN_MS {
                    eprintln!("[{:?}]: WARN: Recognized beat[{}] should is less than {}ms away from the actual value; is: {}ms", strategy, i, DIFF_WARN_MS, abs_diff);
                };
            }
        }
    }

    fn apply_samples_to_all_strategies(window_length: usize, samples: &[i16], _sampling_rate: u32) -> HashMap<StrategyKind, Vec<BeatInfo>> {
        // we pad with zeroes until the audio data length is a multiple
        // of the window length
        let mut samples = Vec::from(samples);
        let remainder = samples.len() % window_length;
        if remainder != 0 {
            samples.extend_from_slice(
                &vec![0; remainder]
            )
        }

        let window_count = samples.len() / window_length;

        // all strategies
        let strategies = vec![
            StrategyKind::LPF,
            StrategyKind::Spectrum,
        ];

        let mut map = HashMap::new();

        for strategy in strategies {
            let detector = strategy.detector(44100, window_length as u16);
            let mut beats = Vec::new();
            for i in 0..window_count {
                let window = &samples[i * window_length..(i+1) * window_length];
                let beat = detector.is_beat(window);
                if let Some(beat) = beat {
                    beats.push(beat);
                }
            }
            map.insert(strategy, beats);
        }

        map
    }

    /// Reads an MP3 and returns the audio data as mono channel + the sampling rate in Hertz.
    fn read_mp3_to_mono(file: &str) -> (Vec<i16>, u32) {
        let mut decoder = Mp3Decoder::new(File::open(file).unwrap());

        let mut sampling_rate = 0;
        let mut mono_samples = vec![];
        loop {
            match decoder.next_frame() {
                Ok(Mp3Frame {
                       data: samples_of_frame,
                       sample_rate,
                       channels,
                       ..
                   }) => {
                    // that's a bird weird of the original API. Why should channels or sampling
                    // rate change from frame to frame?

                    // Should be constant throughout the MP3 file.
                    sampling_rate = sample_rate;

                    if channels == 2 {
                        for (i, sample) in samples_of_frame.iter().enumerate().step_by(2) {
                            let sample = *sample as i32;
                            let next_sample = samples_of_frame[i + 1] as i32;
                            mono_samples.push(
                                ((sample + next_sample) as f32 / 2.0) as i16
                            );
                        }
                    } else if channels == 1 {
                        mono_samples.extend_from_slice(&samples_of_frame);
                    } else {
                        panic!("Unsupported number of channels={}", channels);
                    }


                }
                Err(Mp3Error::Eof) => break,
                Err(e) => panic!("{:?}", e),
            }
        }

        (mono_samples, sampling_rate as u32)
    }
}
