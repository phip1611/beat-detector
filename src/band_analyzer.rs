use crate::audio_history::{AudioHistory, AudioHistoryMeta};
use crate::band_pass_filter::BandPassFilter;
use crate::envelope_detector::{Envelope, EnvelopeDetector};
use crate::util::RingBufferWithSerialSliceAccess;
use biquad::{Biquad, DirectForm1, ToHertz, Type};

/// Continous analyzer for a band-passed audio signal. Must be regularly updated with latest
/// audio samples. It passes them through a band-pass filter and stores the audio internally.
/// On this band-passed audio, the BandAnalyzer detects envelopes with a [`EnvelopeDetector`].
#[derive(Debug)]
pub(crate) struct BandAnalyzer<const SAMPLES_COUNT: usize> {
    buffer: AudioHistory<SAMPLES_COUNT>,
    envelope_detector: EnvelopeDetector,
    band_pass: BandPassFilter,
}

impl<const N: usize> BandAnalyzer<N> {
    /// Constructor.
    pub fn new(lower_frequency: f32, higher_frequency: f32, sampling_frequency: f32) -> Self {
        let band_pass = BandPassFilter::new(lower_frequency, higher_frequency, sampling_frequency);
        let envelope_detector = EnvelopeDetector::new();
        let buffer = AudioHistory::new(sampling_frequency);

        Self {
            band_pass,
            envelope_detector,
            buffer,
        }
    }

    /// Updates the internal state with the latest audio date and tells if an envelope was found.
    pub fn update_and_detect(&mut self, new_samples: &[f32]) -> Option<Envelope> {
        new_samples
            .iter()
            .map(|sample| self.band_pass.apply(*sample))
            .for_each(|sample| self.buffer.update(&[sample]));

        // get slice of band passed data
        let meta = self.buffer.meta();
        let band_passed_samples_slice = self.buffer.latest_audio();

        self.envelope_detector
            .detect_envelope(&meta, band_passed_samples_slice)
    }
}

#[cfg(test)]
mod tests {
    use crate::audio_history::{AudioHistory, AUDIO_HISTORY_DEFAULT_BUFFER_SIZE};
    use crate::band_analyzer::BandAnalyzer;

    use crate::test_util::read_wav_to_mono;
    use crate::util::RingBufferWithSerialSliceAccess;

    #[test]
    fn test_highpass_removes_all_amplitudes() {
        let (audio, wav_header) = read_wav_to_mono("res/sample_1_single_beat.wav");
        // ensure that our file corresponds to the test
        const SAMPLES_COUNT: usize = 14806;

        let mut analyzer =
            BandAnalyzer::<SAMPLES_COUNT>::new(1000.0, 22050.0, wav_header.sampling_rate as f32);

        assert!(analyzer
            .update_and_detect(&audio)
            .is_none());
    }

    #[test]
    fn test_beat_detected_real_audio_single_beat() {
        let (audio, wav_header) = read_wav_to_mono("res/sample_1_single_beat.wav"); // ensure that our file corresponds to the test
        const SAMPLES_COUNT: usize = 14806;

        let mut analyzer =
            BandAnalyzer::<SAMPLES_COUNT>::new(25.0, 70.0, wav_header.sampling_rate as f32);


        // detect envelope; applies the band filter and stores the result
        // in the provided buffer
        let envelope = analyzer
            .update_and_detect(&audio,)
            .unwrap();

        // you can look at the waveform (after a lowpass filter was applied) in audacity and verify these values
        let highest_expected = (0.060, 0.5);
        assert_eq!(highest_expected.0, envelope.highest().relative_time);
        assert_eq!(highest_expected.1, envelope.highest().value);
    }

    #[test]
    fn test_beat_detected_real_audio_sample_1() {
        let (audio, wav_header) = read_wav_to_mono("res/sample_1.wav"); // ensure that our file corresponds to the test

        let mut analyzer = BandAnalyzer::<AUDIO_HISTORY_DEFAULT_BUFFER_SIZE>::new(20.0, 70.0,
            wav_header.sampling_rate as f32,
        );

        let actual = audio
            .chunks(256)
            .map(|samples| {
                analyzer.update_and_detect(samples)
            })
            .flatten()
            .map(|envelope| {
                (
                    envelope.highest().relative_time,
                    envelope.highest().abs_value(),
                )
            })
            .collect::<std::vec::Vec<_>>();

        // I got this values by: dbg!(actual)
        // => I checked in audacity if the values are correct
        let expected = [
            (0.292, 0.535),
            (2.133, 0.424),
            (2.299, 0.505),
            (4.298, 0.514),
            (6.146, 0.424),
            (6.313, 0.472),
        ];

        assert_eq!(actual.len(), expected.len());
        assert_eq!(
            actual
                .into_iter()
                .map(|(time, intensity)| (
                    (time * 1000.0).round() / 1000.0,
                    (intensity * 1000.0).round() / 1000.0
                ))
                .collect::<std::vec::Vec<_>>(),
            expected
        );
    }
}
