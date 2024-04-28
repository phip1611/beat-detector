use crate::EnvelopeInfo;
use crate::{AudioHistory, EnvelopeIterator};
use biquad::{Biquad, Coefficients, DirectForm1, ToHertz, Type, Q_BUTTERWORTH_F32};
use std::fmt::{Debug, Formatter};

/// Cutoff frequency for the lowpass filter to detect beats.
const CUTOFF_FREQUENCY_HZ: f32 = 80.0;

/// Information about a beat.
pub type BeatInfo = EnvelopeInfo;

/// No-op type helper for [`AudioInput`] that is needed as default generic type.
/// Use this when you use [`AudioInput::SliceMono`] or
/// [`AudioInput::SliceStereo`].
#[derive(Debug)]
pub struct StubIterator;

impl Iterator for StubIterator {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

/// The audio input source. Each value must be in range `[-1.0..=1.0]`. This
/// abstraction facilitates the libraries goal to prevent needless copying
/// and buffering of data: internally as well as on a higher level.
pub enum AudioInput<'a, I: Iterator<Item = f32> = StubIterator> {
    /// The audio input stream only consists of mono samples.
    SliceMono(&'a [f32]),
    /// The audio input streams consists of interleaved samples following a
    /// LRLRLR or RLRLRL scheme. This is typically the case for stereo channel
    /// audio. Internally, the audio will be combined to a mono track.
    SliceStereo(&'a [f32]),
    /// Custom iterator emitting mono samples in f32 format.
    Iterator(I),
}

impl<'a> AudioInput<'a, StubIterator> {
    /// Creates an empty input. Useful if you want to run the detection again
    /// without adding more data.
    pub const fn empty() -> Self {
        Self::SliceMono(&[])
    }
}

impl<'a, I: Iterator<Item = f32>> Debug for AudioInput<'a, I> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let variant = match self {
            AudioInput::SliceMono(_) => "SliceMono(data...)",
            AudioInput::SliceStereo(_) => "SliceStereo(data...)",
            AudioInput::Iterator(_) => "Iterator(data...)",
        };
        f.debug_tuple("AudioInput").field(&variant).finish()
    }
}

#[derive(Debug)]
pub struct BeatDetector {
    lowpass_filter: DirectForm1<f32>,
    /// Whether the lowpass filter should be applied. Usually you want to
    /// set this to true. Set it to false if you know that all your audio
    /// input already only contains the interesting frequencies to save some
    /// computations.
    needs_lowpass_filter: bool,
    history: AudioHistory,
    /// Holds the previous beat. Once this is initialized, it is never `None`.
    previous_beat: Option<BeatInfo>,
}

impl BeatDetector {
    pub fn new(sampling_frequency_hz: f32, needs_lowpass_filter: bool) -> Self {
        let lowpass_filter = Self::create_lowpass_filter(sampling_frequency_hz);
        Self {
            lowpass_filter,
            needs_lowpass_filter,
            history: AudioHistory::new(sampling_frequency_hz),
            previous_beat: None,
        }
    }

    /// Consumes the latest audio data and returns if the audio history,
    /// consisting of previously captured audio and the new data, contains a
    /// beat. This function is supposed to be frequently
    /// called everytime new audio data from the input source is available so
    /// that:
    /// - the latency is low
    /// - no beats are missed
    ///
    /// From experience, Linux audio input libraries give you a 20-40ms audio
    /// buffer every 20-40ms with the latest data. That's a good rule of thumb.
    ///
    /// If new audio data contains two beats, only the first one will be
    /// discovered. On the next invocation, the next beat will be discovered,
    /// if still present in the internal audio window.
    pub fn update_and_detect_beat(
        &mut self,
        input: AudioInput<impl Iterator<Item = f32>>,
    ) -> Option<BeatInfo> {
        self.consume_audio(input);

        let search_begin_index = self
            .previous_beat
            .and_then(|info| self.history.total_index_to_index(info.to.total_index));
        // Envelope iterator with respect to previous beats.
        let mut envelope_iter = EnvelopeIterator::new(&self.history, search_begin_index);
        let beat = envelope_iter.next();
        if let Some(beat) = beat {
            self.previous_beat.replace(beat);
        }
        beat
    }

    /// Applies the data from the given [`AudioInput`] to the lowpass filter and
    /// adds it to the internal audio window.
    fn consume_audio(&mut self, input: AudioInput<impl Iterator<Item = f32>>) {
        match input {
            AudioInput::SliceMono(slice) => {
                let iter = slice.iter().map(|&sample| {
                    if self.needs_lowpass_filter {
                        self.lowpass_filter.run(sample)
                    } else {
                        sample
                    }
                });
                self.history.update(iter)
            }
            AudioInput::SliceStereo(slice) => {
                let iter = slice
                    .chunks(2)
                    .map(|lr| (lr[0] + lr[1]) / 2.0)
                    .map(|sample| {
                        if self.needs_lowpass_filter {
                            self.lowpass_filter.run(sample)
                        } else {
                            sample
                        }
                    });

                self.history.update(iter)
            }
            AudioInput::Iterator(iter) => {
                let iter = iter.map(|sample| {
                    if self.needs_lowpass_filter {
                        self.lowpass_filter.run(sample)
                    } else {
                        sample
                    }
                });
                self.history.update(iter)
            }
        }
    }

    fn create_lowpass_filter(sampling_frequency_hz: f32) -> DirectForm1<f32> {
        // Cutoff frequency.
        let f0 = CUTOFF_FREQUENCY_HZ.hz();
        // Samling frequency.
        let fs = sampling_frequency_hz.hz();

        let coefficients =
            Coefficients::<f32>::from_params(Type::LowPass, fs, f0, Q_BUTTERWORTH_F32).unwrap();
        DirectForm1::<f32>::new(coefficients)
    }
}

#[cfg(test)]
mod tests {
    use crate::BeatDetector;

    #[test]
    fn is_send_and_sync() {
        fn accept<I: Send + Sync>() {}

        accept::<BeatDetector>();
    }
}

/// Here, I run "static" tests against the beat detector. This means, the whole
/// waveform is loaded at once into the internal buffer. I expect the same
/// results as for the EnvelopeIterator unit tests.
#[cfg(test)]
mod static_data_tests {
    use super::*;
    use crate::{test_utils, SampleInfo};
    use ringbuffer::RingBuffer;
    use std::time::Duration;

    /// Tests that the audio input consomption with all possible variants
    /// properly works.
    #[test]
    fn audio_input_consumption_works() {
        let mut detector = BeatDetector::new(1000.0, false);
        assert_eq!(detector.history.data().len(), 0);

        let input = AudioInput::<StubIterator>::SliceMono(&[0.1, 0.2]);
        let _ = detector.update_and_detect_beat(input);
        assert_eq!(detector.history.data().len(), 2);

        let input = AudioInput::<StubIterator>::SliceStereo(&[0.1, 0.2]);
        let _ = detector.update_and_detect_beat(input);
        assert_eq!(detector.history.data().len(), 3);

        let input = AudioInput::<_>::Iterator([0.3].iter().copied());
        let _ = detector.update_and_detect_beat(input);
        assert_eq!(detector.history.data().len(), 4);

        assert_eq!(detector.history.data()[0], 0.1);
        assert_eq!(detector.history.data()[1], 0.2);
        assert_eq!(
            detector.history.data()[2],
            0.15 /* calculated mono value */
        );
        assert_eq!(detector.history.data()[3], 0.3);
    }

    #[test]
    #[allow(non_snake_case)]
    fn detect__static__no_lowpass__holiday_single_beat() {
        let (samples, header) = test_utils::samples::holiday_single_beat();
        let mut detector = BeatDetector::new(header.sampling_rate as f32, false);
        let input = AudioInput::<StubIterator>::SliceMono(samples.as_slice());
        assert_eq!(
            detector.update_and_detect_beat(input),
            Some(EnvelopeInfo {
                from: SampleInfo {
                    value: 0.11386456,
                    index: 256,
                    total_index: 256,
                    timestamp: Duration::from_secs_f32(0.005804989),
                    duration_behind: Duration::from_secs_f32(0.401904759)
                },
                to: SampleInfo {
                    value: 0.39106417,
                    index: 1971,
                    total_index: 1971,
                    timestamp: Duration::from_secs_f32(0.044693876),
                    duration_behind: Duration::from_secs_f32(363.015872),
                },
                max: SampleInfo {
                    value: -0.6453749,
                    index: 830,
                    total_index: 830,
                    timestamp: Duration::from_secs_f32(18.820861),
                    duration_behind: Duration::from_secs_f32(388.888887),
                }
            })
        );
        assert_eq!(detector.update_and_detect_beat(AudioInput::empty()), None);
    }
    /*
    #[test]
    fn find_beats_sample1_double_beat() {
        let (samples, header) = test_utils::samples::sample1_double_beat();
        let mut detector = BeatDetector::new(header.sampling_rate as f32);
        let input = AudioInput::<StubIterator>::SliceMono(samples.as_slice());
        assert_eq!(
            detector.detect_beat(input),
            Some(EnvelopeInfo {
                from: SampleInfo {
                    value: -0.21601434,
                    index: 822,
                    total_index: 822,
                    timestamp: Duration::from_secs_f32(0.018639455),
                    duration_behind: Duration::from_secs_f32(0.398208608)
                },
                to: SampleInfo {
                    value: -0.10831339,
                    index: 17415,
                    total_index: 17415,
                    timestamp: Duration::from_secs_f32(0.394897938),
                    duration_behind: Duration::from_secs_f32(0.021950125)
                },
                max: SampleInfo {
                    value: 0.41667685,
                    index: 9133,
                    total_index: 9133,
                    timestamp: Duration::from_secs_f32(0.207097501),
                    duration_behind: Duration::from_secs_f32(0.209750562)
                }
            })
        );
        let input = AudioInput::<StubIterator>::SliceMono([].as_slice());
        assert_eq!(
            detector.detect_beat(input),
            Some(EnvelopeInfo {
                from: SampleInfo {
                    value: -0.21601434,
                    index: 822,
                    total_index: 822,
                    timestamp: Duration::from_secs_f32(0.018639455),
                    duration_behind: Duration::from_secs_f32(0.398208608)
                },
                to: SampleInfo {
                    value: -0.10831339,
                    index: 17415,
                    total_index: 17415,
                    timestamp: Duration::from_secs_f32(0.394897938),
                    duration_behind: Duration::from_secs_f32(0.021950125)
                },
                max: SampleInfo {
                    value: 0.41667685,
                    index: 9133,
                    total_index: 9133,
                    timestamp: Duration::from_secs_f32(0.207097501),
                    duration_behind: Duration::from_secs_f32(0.209750562)
                }
            })
        );
        let input = AudioInput::<StubIterator>::SliceMono([].as_slice());
        assert_eq!(
            detector.detect_beat(input),
            None /* there are only two beats in that sample */
        );
    }*/
}
