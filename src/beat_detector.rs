use crate::EnvelopeInfo;
use crate::{AudioHistory, EnvelopeIterator};
use biquad::{Biquad, Coefficients, DirectForm1, ToHertz, Type, Q_BUTTERWORTH_F32};
use std::fmt::{Debug, Formatter};

/// Cutoff frequency for the lowpass filter to detect beats.
const CUTOFF_FREQUENCY_HZ: f32 = 70.0;

/// Information about a beat.
pub type BeatInfo = EnvelopeInfo;

/// The audio input source. Each value must be in range `[-1.0..=1.0]`. This
/// abstraction facilitates the libraries goal to prevent needless copying
/// and buffering of data: internally as well as on a higher level.
pub enum AudioInput<'a, I: Iterator<Item = f32>> {
    /// The audio input stream only consists of mono samples.
    SliceMono(&'a [f32]),
    /// The audio input streams consists of interleaved samples following a
    /// LRLRLR or RLRLRL scheme. This is typically the case for stereo channel
    /// audio. Internally, the audio will be combined to a mono track.
    SliceStereo(&'a [f32]),
    /// Custom iterator emitting mono samples in f32 format.
    Iterator(I),
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
    history: AudioHistory,
}

impl BeatDetector {
    pub fn new(sampling_frequency_hz: f32) -> Self {
        let lowpass_filter = BeatDetector::create_lowpass_filter(sampling_frequency_hz);
        Self {
            lowpass_filter,
            history: AudioHistory::new(sampling_frequency_hz),
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
    pub fn detect_beat<'a>(
        &mut self,
        input: AudioInput<'a, impl Iterator<Item = f32>>,
    ) -> Option<BeatInfo> {
        match input {
            AudioInput::SliceMono(slice) => {
                let iter = slice.iter().map(|&sample| self.lowpass_filter.run(sample));
                self.history.update(iter)
            }
            AudioInput::SliceStereo(slice) => {
                let iter = slice
                    .chunks(2)
                    .map(|lr| (lr[0] + lr[1]) / 2.0)
                    .map(|sample| self.lowpass_filter.run(sample));

                self.history.update(iter)
            }
            AudioInput::Iterator(iter) => self.history.update(iter),
        }

        // TODO prevent detection of same beat
        let mut envelope_iter = EnvelopeIterator::new(&self.history, None);
        envelope_iter.next()
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
