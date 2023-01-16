use crate::audio_history::{AudioHistory, AUDIO_HISTORY_DEFAULT_BUFFER_SIZE, AudioHistoryMeta};
use crate::band_analyzer::BandAnalyzer;
use crate::beat_info::FrequencyBand;
use crate::envelope_detector::Envelope;
use crate::util::RingBufferWithSerialSliceAccess;
use crate::BeatInfo;
use core::cell::Cell;

/// Beat Analyzer that operates on f32 mono audio data. It keeps a history of the audio data
/// to improve analysis. Works entirely on the heap.
///
/// The sampling rate must stay equal during the recording. In case your audio recording records
/// in i16 format, please make sure to transform the audio data to f32 and scale it accordingly
/// into interval `[-1, 1]`.
///
/// Uses biquad filters to find beats in several frequency bands. Thus, it can find low beats
/// (drums/bass) or high beats (claps).
#[derive(Debug)]
pub struct BeatDetector {
    /// Contains meta about the recorded history of audio data.
    /// Always in sync with the [`AudioHistoryMeta`] in the corresponding instances of
    /// [`BandAnalyzer`]. Lives inside the beat detector struct only for convenience. It is not
    /// necessary to keep a copy of the original audio here. It is more efficient that the
    /// [`BandAnalyzer`]
    audio_history_meta: AudioHistoryMeta,
    /// Information about the previous 10 beats (if present).
    /// New elements are always `Some(T)`.
    // TODO use "ringbuffer"-crate or so instead of my specialized audio ring buffer impl
    // beat_history: RingBufferWithSerialSliceAccess<Option<Envelope>, 10>,
    /// Tells if the value range was asserted.
    // TODO remove
    assert_values_done: Cell<bool>,
    /// Analyzer that checks the input data for low frequency beats (bass).
    // The BandAnalyzer needs internal state; thus we can not recreate it on every callback
    low_band_analyzer: BandAnalyzer<AUDIO_HISTORY_DEFAULT_BUFFER_SIZE>,
}

impl BeatDetector {
    /// Constructor.
    pub fn new(sampling_rate: f32) -> Self {
        let detector = Self {
            audio_history_meta: AudioHistoryMeta::new(AUDIO_HISTORY_DEFAULT_BUFFER_SIZE, sampling_rate),
            assert_values_done: Cell::new(false),
            low_band_analyzer: BandAnalyzer::new(20.0, 70.0, sampling_rate),
        };
        log::trace!(
            "BeatDetector consumes {} on the stack",
            core::mem::size_of_val(&detector)
        );
        detector
    }

    /// Callback on new audio data. Analyzes the next amount of samples (including the new samples)
    /// and returns if a beat was detected or not. The audio data must be in mono format! In case
    /// your audio recording records in i16 format, please make sure to transform it to f32 and
    /// scale it into interval `[-1, 1]`.
    ///
    /// Please make sure that for a low latency the length of the slice needs to be small
    /// (not more than 4096 bytes). Small new amounts of data (<100 new samples) are also fine
    /// for the algorithm.
    ///
    /// The detector keeps an internal state of the ongoing relative time. The ongoing relative
    /// time is determined by the amount of samples and the time per sample.
    ///
    /// The underlying [`BandAnalyzer`] ensures that the same beat is never detected twice.
    pub fn on_new_audio(&mut self, new_audio_data: &[f32]) -> Option<BeatInfo> {
        self.assert_new_audio_data(new_audio_data);
        // updates internal time stats etc.
        self.audio_history_meta.update(new_audio_data);

        let envelope = self.low_band_analyzer.update_and_detect(
            new_audio_data,
        )?;

        // TODO replace this by a better data structure.. odd to use an option here :(
        //self.beat_history.push(Some(envelope));

        // todo calc bpm
        Some(envelope).map(|env| BeatInfo::new(1, FrequencyBand::Low, env))
    }

    /// Certain assertions regarding the new audio data.
    fn assert_new_audio_data(&self, new_audio_data: &[f32]) {
        // Only do this once to speed up processing. It is really unlikely that
        // this changes during the running of the algorithm.
        if !self.assert_values_done.get() {
            self.assert_values_done.replace(true);
            assert!(
                !new_audio_data.iter().any(|x| *x > 1.0 && *x < -1.0),
                "all samples must be in range [-1; 1]"
            );
        }

        if new_audio_data.len() < 50 {
            log::warn!(
                "the audio window is really small with {} samples",
                new_audio_data.len()
            );
        } else if new_audio_data.len() as f32 * self.audio_history_meta.time_per_sample() as f32 > 0.1 {
            log::warn!(
                "the audio window is really big with {} samples (more than 100ms!)",
                new_audio_data.len()
            );
        }
    }

    /*fn count_bpm(&self) -> u8 {
        let beats = self.beat_history.
        self.beat_history.
    }*/
}
#[allow(clippy::float_cmp)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::read_wav_to_mono;
    use std::vec::Vec;

    #[ignore]
    #[test]
    fn test_sample_1_print_beats() {
        let (sample_1_audio_data, wav_header) =
            read_wav_to_mono("res/ausschnitt-holiday-lowpassed.wav");

        let window_length = 256;

        let beats = apply_samples_to_beat_detector(
            window_length,
            wav_header.sampling_rate as f32,
            &sample_1_audio_data,
        );

        for beat in beats {
            println!("  {:.6}s", beat.envelope().highest().relative_time);
        }
    }

    #[test]
    fn test_sample_1_beat_detection() {
        let (sample_1_audio_data, wav_header) = read_wav_to_mono("res/sample_1.wav");

        let window_length = 256;

        // opened the file in Audacity and looked where the beats are
        // each time stamp marks the beginning of the beat.
        // Therefore, we must allow a small delta.
        const SAMPLE_1_EXPECTED_BEATS_MS: [f32; 6] = [0.300, 2.131, 2.297, 4.303, 6.143, 6.310];

        let beats = apply_samples_to_beat_detector(
            window_length,
            wav_header.sampling_rate as f32,
            &sample_1_audio_data,
        );

        assert_eq!(beats.len(), SAMPLE_1_EXPECTED_BEATS_MS.len());

        for (actual_beat, expected_beat_time) in beats.iter().zip(SAMPLE_1_EXPECTED_BEATS_MS.iter())
        {
            assert!((actual_beat.time_of_beat() - *expected_beat_time).abs() < 0.01);
        }
    }

    fn apply_samples_to_beat_detector(
        window_length: usize,
        sampling_rate: f32,
        samples: &[f32],
    ) -> Vec<BeatInfo> {
        let mut detector = BeatDetector::new(sampling_rate);
        samples
            .chunks(window_length)
            .map(|chunk| detector.on_new_audio(chunk))
            .flatten()
            .collect()
    }
}
