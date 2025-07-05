//! Actual beat detection combining all other building blocks.

use crate::layer_analysis::audio_history::{AudioHistory, SampleInfo};
use crate::layer_analysis::max_min_iterator::MaxMinIterator;
use crate::layer_input_processing::conversion::{f32_sample_to_i16_unchecked, i16_sample_to_f32};
use crate::layer_input_processing::downsampling::Downsampler;
use crate::layer_input_processing::lowpass_filter::LowpassFilter;
use crate::{DownsamplingMetrics, ValidInputFrequencies};
use ringbuffer::RingBuffer;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BeatInfo {
    sample_info: SampleInfo,
}

/// Beat detector.
#[derive(Debug)]
pub struct BeatDetector {
    // read-only fields
    downsampling_metrics: DownsamplingMetrics,
    frequencies: ValidInputFrequencies,
    // mutable fields
    downsampler: Downsampler,
    history: AudioHistory,
    lowpass_filter: LowpassFilter,
    previous_beat_info: Option<BeatInfo>,
}

impl BeatDetector {
    const BEAT_MIN_DISTANCE: Duration = Duration::from_millis(60);

    /// Creates a new beat detector.
    pub fn new(frequencies: ValidInputFrequencies) -> Self {
        let metrics = DownsamplingMetrics::new(frequencies.clone());
        let downsampler = Downsampler::new(metrics.clone());
        let lowpass_filter = LowpassFilter::new(frequencies.clone());
        let history = AudioHistory::new(metrics.effective_sample_rate_hz(), Some(metrics.clone()));

        Self {
            downsampler,
            downsampling_metrics: metrics,
            frequencies,
            history,
            lowpass_filter,
            previous_beat_info: None,
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
    /// From experience, Linux audio input libraries (using ALSA as backend)
    /// give you a 20-40ms audio buffer every 20-40ms with the latest data.
    /// That's a good rule of thumb. This corresponds to 1800 mono samples at a
    /// sampling rate of 44.1kHz.
    ///
    /// If new audio data contains two beats, only the first one will be
    /// discovered. On the next invocation, the next beat will be discovered,
    /// if still present in the internal audio window.
    pub fn update_and_detect_beat(
        &mut self,
        mono_samples_iter: impl ExactSizeIterator<Item = i16>,
    ) -> Option<BeatInfo> {
        self.consume_audio(mono_samples_iter);
        self.detect_beat()
    }

    /// Consumes the audio, applies the lowpass filter, performs internal
    /// downsampling, and stores the result in the internal audio history.
    fn consume_audio(&mut self, mono_samples_iter: impl ExactSizeIterator<Item = i16>) {
        let iter = mono_samples_iter
            .map(i16_sample_to_f32)
            // Apply lowpass filter.
            .map(|sample| self.lowpass_filter.process(sample))
            // SAFETY: We know that the values are all valid at this
            // point. This is the hot path, so we want to be quick.
            .map(|sample| unsafe { f32_sample_to_i16_unchecked(sample) });
        
        let downsample_iter = self.downsampler.downsample(iter);
        
        self.history.update(iter);
    }

    /// Tries to detect a beat in the internal audio history.
    // TODO replace by much more robust algorithm.
    fn detect_beat(&mut self) -> Option<BeatInfo> {
        let max_min_iter = MaxMinIterator::new(&self.history, None);

        let peaks_sum = max_min_iter
            .clone()
            .map(|info| info.amplitude.abs() as u64)
            .sum::<u64>();
        let peaks_count = max_min_iter.clone().count() as u64;
        let peaks_max = max_min_iter.max_by_key(|info| info.amplitude.abs() as u64)?;

        if peaks_count == 0 {
            return None;
        }

        let peaks_avg = peaks_sum / peaks_count;
        let peaks_max_amplitude = peaks_max.amplitude.abs() as u64;
        let threshold = peaks_avg * 3 / 4;
        if peaks_max_amplitude > threshold {
            if let Some(last_beat) = self.previous_beat_info.as_ref() {
                let dur = peaks_max.timestamp - last_beat.sample_info.timestamp;
                if dur < Self::BEAT_MIN_DISTANCE {
                    return None;
                }
            }

            let beat_info = BeatInfo {
                sample_info: peaks_max,
            };
            self.previous_beat_info.replace(beat_info.clone());
            return Some(beat_info);
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils;
    use std::vec::Vec;

    fn simulate_dynamic_audio_source(
        chunk_size: usize,
        samples: &[i16],
        detector: &mut BeatDetector,
    ) -> Vec<usize> {
        samples
            .chunks(chunk_size)
            .flat_map(|samples| {
                detector
                    .update_and_detect_beat(samples.iter().copied())
                    .map(|info| info.sample_info.total_index_original)
            })
            .collect::<std::vec::Vec<_>>()
    }

    #[test]
    fn test_beat_detection_sample1_single_beat() {
        let (samples, header) = test_utils::samples::sample1_single_beat();

        let frequencies = ValidInputFrequencies::new(header.sample_rate as f32, 100.0).unwrap();
        let mut detector = BeatDetector::new(frequencies);

        // assuming all data fits into the internal buffer
        let beat = detector.update_and_detect_beat(samples.iter().copied());

        assert_eq!(
            beat,
            Some(BeatInfo {
                sample_info: SampleInfo {
                    amplitude: 0,
                    index: 0,
                    total_index: 0,
                    total_index_original: 0,
                    timestamp: Default::default(),
                    duration_behind: Default::default(),
                },
            })
        );
    }

    #[test]
    fn test_beat_detection_1() {
        let (samples, header) = test_utils::samples::sample1_long();

        let frequencies = ValidInputFrequencies::new(header.sample_rate as f32, 100.0).unwrap();
        let mut detector = BeatDetector::new(frequencies);
        assert_eq!(
            simulate_dynamic_audio_source(256, &samples, &mut detector),
            &[]
        );
    }
}
