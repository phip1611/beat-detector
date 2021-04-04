use crate::strategies::{lpf, spectrum};
use crate::strategies::lpf::LpfBeatDetector;

mod strategies;

/// Struct that holds information about a detected beat.
#[derive(Debug)]
pub struct BeatInfo {
    relative_ms: u32,
    // todo intensity
}
impl BeatInfo {
    pub fn new(relative_ms: u32) -> Self {
        Self {
            relative_ms,
        }
    }
}

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
}

/// Enum that conveniently and easily makes all [`Strategy`]s provided by this crate accessible.
#[derive(Debug)]
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
    fn detector(&self, sampling_rate: u32, window_length: u16) -> impl Strategy {
        match self {
            StrategyKind::LPF => LpfBeatDetector::new(sampling_rate, window_length),
            StrategyKind::Spectrum => todo!(),
            _ => panic!("Unknown Strategy")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use minimp3::{Decoder as Mp3Decoder, Error as Mp3Error, Frame as Mp3Frame};
    use std::fs::File;


    #[test]
    fn test_sample_1_print_beats() {
        let (mut sample_1_audio_data, sampling_rate) = read_mp3_to_mono("res/sample_1.mp3");
        // assert 44,1kHz because it makes things easier
        assert_eq!(sampling_rate, 44100, "The sampling rate of the MP3 examples must be 44100Hz.");

        // 1/44100 * 2048 == 2048/44100 == 0.046439s == 46.439ms
        let window_length = 2048;

        // we pad with zeroes until the audio data length is a multiple
        // of the window length
        let remainder = sample_1_audio_data.len() % window_length;
        if remainder != 0 {
            sample_1_audio_data.extend_from_slice(
                &vec![0; remainder]
            )
        }
        let window_count = sample_1_audio_data.len() / window_length;

        // all strategies
        let strategies = vec![
            StrategyKind::LPF,
            // not implemented yet
            // StrategyKind::Spectrum,
        ];

        for strategy in strategies {
            let detector = strategy.detector(44100, window_length as u16);
            let mut beats = Vec::new();
            for i in 0..window_count {
                let window = &sample_1_audio_data[i * window_length..(i+1) * window_length];
                beats.push(
                    detector.is_beat(window)
                );
            }
            println!("{:#?}", beats.iter().filter(|x| x.is_some()).collect::<Vec<&Option<BeatInfo>>>());
        }
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
