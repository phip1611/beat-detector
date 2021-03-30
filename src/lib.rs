use crate::strategies::{lpf, spectrum};

mod strategies;

#[derive(Debug)]
pub struct BeatInfo {
    relative_ms: u32,
}

pub trait Strategy {
    fn is_beat(&self, samples: &[i16], sampling_rate: u16, counter: u32) -> Option<BeatInfo>;
}

/// Strategy to obtain beats from samples.
#[derive(Debug)]
pub enum StrategyKind {
    /// Lowpass-Filter
    LPF,
    /// Frequency analysis
    Spectrum,
}

impl Strategy for StrategyKind {
    fn is_beat(&self, samples: &[i16], sampling_rate: u16, counter: u32) -> Option<BeatInfo> {
        let is_beat = match self {
            StrategyKind::LPF => lpf::is_beat(samples, sampling_rate),
            StrategyKind::Spectrum => spectrum::is_beat(samples, sampling_rate),
            _ => panic!("Unknown Strategy")
        };

        if !is_beat {
            None
        } else {
            let sampling_rate = sampling_rate as f32;
            // Assumes the samples has always the same length during a single run
            let samples_len = samples.len() as f32;
            let ms_per_sample = 1.0 / sampling_rate * 1000.0; // Hertz to Seconds to Milliseconds
            let ms_of_sample = ms_per_sample * samples_len;
            let relative_time_ms = (ms_of_sample * counter as f32) as u32;
            Some(
                BeatInfo {
                    relative_ms: relative_time_ms
                }
            )
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
            let mut beats = Vec::new();
            for i in 0..window_count {
                let window = &sample_1_audio_data[i * window_length..(i+1) * window_length];
                beats.push(
                    strategy.is_beat(
                        window,
                        44100,
                        i as u32
                    )
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
