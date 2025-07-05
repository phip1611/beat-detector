//! Actual beat detection combining all other building blocks.

use crate::audio_analysis::max_min_iterator::MaxMinIterator;
use crate::audio_preprocessing::audio_history::{AudioHistory, SampleInfo};
use crate::audio_preprocessing::downsampling::{Downsampler, DownsamplingMetrics};
use crate::audio_preprocessing::lowpass_filter::LowpassFilter;
use crate::audio_preprocessing::{lowpass_and_downsample_i16_samples_iter, ValidInputFrequencies};
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
    history: AudioHistory,
    /// downsampler
    downsampler: Downsampler,
    lowpass_filter: LowpassFilter,
    previous_beat_info: Option<BeatInfo>,
}

impl BeatDetector {
    const BEAT_MIN_DISTANCE: Duration = Duration::from_millis(60);

    /// Creates a new beat detector.
    ///
    /// # Arguments
    /// - `frequencies`: The input frequencies of the signal.
    /// - `do_lowpass`: Whether the lowpass filter should be applied. Disable
    ///   this only if the input signal is guaranteed to already be passed
    ///   through a lowpass filter.
    /// - `do_downsample`: Whether the input data should be downsampled. This
    ///   typically should only be set to `false` for debugging and testing.
    pub fn new(frequencies: ValidInputFrequencies, do_lowpass: bool, do_downsample: bool) -> Self {
        let metrics = if do_downsample {
            DownsamplingMetrics::new(frequencies.clone())
        } else {
            DownsamplingMetrics::new_disabled(frequencies.clone())
        };

        let lowpass_filter = if do_lowpass {
            LowpassFilter::new(frequencies.clone())
        } else {
            LowpassFilter::new_passthrough(frequencies.clone())
        };

        let downsampler = Downsampler::new(metrics.clone());

        let history = AudioHistory::new(
            metrics.effective_sample_rate_hz(),
            Some(metrics.clone()),
            None,
        );

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
        mono_samples_iter: impl Iterator<Item = i16> + Clone,
    ) -> Option<BeatInfo> {
        self.consume_audio(mono_samples_iter);
        self.detect_beat()
    }

    /// Consumes the audio, applies the lowpass filter, performs internal
    /// downsampling, and stores the result in the internal audio history.
    fn consume_audio(&mut self, mono_samples_iter: impl Iterator<Item = i16>) {
        let iter = lowpass_and_downsample_i16_samples_iter(
            mono_samples_iter,
            &mut self.lowpass_filter,
            &mut self.downsampler,
        );
        self.history.update(iter);
    }

    /// Tries to detect a beat in the internal audio history.
    // TODO replace by much more robust algorithm.
    fn detect_beat(&mut self) -> Option<BeatInfo> {
        let max_min_iter = MaxMinIterator::new(&self.history, None);

        let foo = max_min_iter.clone().collect::<std::vec::Vec<_>>();
        panic!("{foo:#?}");

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
        let mut detector = BeatDetector::new(frequencies, true, true);

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
        /*let (samples, header) = test_utils::samples::sample1_long();

        let frequencies = ValidInputFrequencies::new(header.sample_rate as f32, 100.0).unwrap();
        let mut detector = BeatDetector::new(frequencies);
        assert_eq!(
            simulate_dynamic_audio_source(256, &samples, &mut detector),
            &[]
        );*/
    }
}
