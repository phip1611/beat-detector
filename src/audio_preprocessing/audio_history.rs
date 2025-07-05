/*
MIT License

Copyright (c) 2024 Philipp Schuster

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
*/

//! Helpers for audio history bookkeeping.
//!
//! We need to get new audio samples as we go but keep knowledge for old ones.
//! A short period of history is needed to properly
//!
//! See [`AudioHistory`] and [`SampleInfo`].

use crate::audio_preprocessing::downsampling::DownsamplingMetrics;
use crate::audio_preprocessing::f32::F32Frequency;
use core::cmp::Ordering;
use core::time::Duration;
use ringbuffer::{ConstGenericRingBuffer, RingBuffer};
use crate::audio_preprocessing::lowpass_filter::LowpassFilter;

/// The default buffer size for the audio history with sufficient safety
/// capacity.
///
/// As this crate stores the audio data in mono channel i16 format and performs
/// downsampling, we will have effective sampling rates of 500 Hz and below.
/// To keep samples of the last [`MIN_WINDOW`], 1024 is sufficient.
///
/// You can run `cargo run --bin downsample-bufsize-helper` to get insights on what we
/// need here.
#[cfg(not(test))]
const DEFAULT_BUFFER_SIZE: usize = 1024;
#[cfg(test)] // in tests, I often don't downsample things => more memory needed
const DEFAULT_BUFFER_SIZE: usize = 44100;

/// Minimum window size (duration) for the audio buffer to do proper beat
/// detection.
pub const MIN_WINDOW: Duration = Duration::from_millis(300);

/// Sample info with time context.
#[derive(Copy, Clone, Debug, Default)]
pub struct SampleInfo {
    /// The value (amplitude) of the sample.
    pub amplitude: i16,
    /// The current index in [`AudioHistory`].
    pub index: usize,
    /// The total index since the beginning of audio history.
    pub total_index: usize,
    /// The total index since the beginning of audio history but adjusted
    /// to the original audio, i.e., without the downsampling simplification.
    pub total_index_original: usize,
    /// Relative timestamp since the beginning of audio history.
    pub timestamp: Duration,
    /// The time the sample is behind the latest data.
    pub duration_behind: Duration,
}

impl PartialEq for SampleInfo {
    fn eq(&self, other: &Self) -> bool {
        self.total_index.eq(&other.total_index)
    }
}

impl PartialOrd for SampleInfo {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for SampleInfo {}

impl Ord for SampleInfo {
    fn cmp(&self, other: &Self) -> Ordering {
        self.total_index.cmp(&other.total_index)
    }
}

/// Accessor over the captured audio history that helps to identify the
/// timestamp of each sample.
///
/// Users are supposed to add new data in chunks that are less than the buffer
/// size, to slowly fade out old data from the underlying ringbuffer.
#[derive(Debug)]
pub struct AudioHistory {
    // read-only properties
    
    /// Time per sample regarding the effective sample rate.
    time_per_sample: f32,
    /// If a lowpass filter is set, this specifies the group delay.
    /// This influences the total index in the original audio source.
    filter_group_delay: Option<usize>,
    // If downsampling was used, the metrics to adjust some things.
    downsampling_metrics: Option<DownsamplingMetrics>,
    
    // mutable properties
    audio_buffer: ConstGenericRingBuffer<i16, DEFAULT_BUFFER_SIZE>,
    total_consumed_samples: usize,
}

impl AudioHistory {
    /// Creates a new audio history buffer.
    ///
    /// The sample rate has to be the effective rate with that the audio
    /// buffer is fed, such as the effective sample rate after downsampling.
    // todo constructor is weird
    pub fn new(
        // Original sample rate
        original_sample_rate_hz: F32Frequency,
        // Metrics with possibly now effective sample rate.
        downsampling_metrics: Option<DownsamplingMetrics>,
        // Group delay of the lowpass filter, if any is used.
        filter_group_delay: Option<usize>
    ) -> Self {
        let audio_buffer = ConstGenericRingBuffer::new();
        
        // The effective sample rate as seen by the audio history.
        let sample_rate_hz = downsampling_metrics
            .as_ref()
            .map(|metrics| metrics.effective_sample_rate_hz())
            .unwrap_or(original_sample_rate_hz);
        
        Self {
            audio_buffer,
            time_per_sample: 1.0 / sample_rate_hz.raw(),
            total_consumed_samples: 0,
            downsampling_metrics,
            filter_group_delay
        }
    }

    /// Updates the audio history with fresh samples. The audio samples are
    /// expected to be in mono channel format.
    #[inline]
    pub fn update(&mut self, mono_samples_iter: impl Iterator<Item = i16>) {
        let mut len = 0;
        for sample in mono_samples_iter {
            self.audio_buffer.push(sample);
            self.total_consumed_samples += 1;
        }

        self.total_consumed_samples += len;

        if len >= self.audio_buffer.capacity() {
            log::warn!(
                "Adding {} samples to the audio buffer that only has a capacity for {} samples.",
                len,
                self.audio_buffer.capacity()
            );
            #[cfg(test)]
            std::eprintln!(
                "WARN: AudioHistory::update: Adding {} samples to the audio buffer that only has a capacity for {} samples.",
                len,
                self.audio_buffer.capacity()
            );
        }
    }

    /// Get the passed time in seconds.
    #[inline]
    pub fn passed_time(&self) -> Duration {
        let seconds = self.time_per_sample * self.total_consumed_samples as f32;
        Duration::from_secs_f32(seconds)
    }

    /// Access the underlying data storage.
    #[inline]
    pub const fn data(&self) -> &ConstGenericRingBuffer<i16, DEFAULT_BUFFER_SIZE> {
        &self.audio_buffer
    }

    /// Returns the [`SampleInfo`] about a sample from the current index of that
    /// sample.
    #[inline]
    pub fn index_to_sample_info(&self, index: usize) -> SampleInfo {
        assert!(index < self.data().capacity());

        let timestamp = self.timestamp_of_index(index);
        let value = self.data()[index];
        let total_index = self.index_to_sample_number(index);
        let total_index_original = total_index
            * self
                .downsampling_metrics
                .as_ref()
                .map(|metrics| metrics.factor())
                .unwrap_or(1);
        // Shift back in time according to the group delay.
        let total_index_original = total_index_original.saturating_sub(self.filter_group_delay.unwrap_or(0));
        
        SampleInfo {
            index,
            timestamp,
            amplitude: value,
            total_index,
            total_index_original,
            duration_behind: self.timestamp_of_index(self.data().len() - 1) - timestamp,
        }
    }

    /// Returns the index in the current captured audio window from the total
    /// index of the given sample, if present.
    #[inline]
    pub fn total_index_to_index(&self, total_index: usize) -> Option<usize> {
        // TODO this looks way too complicated. Probably can be simplified.
        if self.lost_samples() == 0 {
            if total_index < self.total_consumed_samples {
                Some(total_index)
            } else {
                None
            }
        } else if total_index < self.lost_samples() {
            None
        } else {
            let index = total_index - self.lost_samples();
            if index <= self.data().capacity() {
                Some(index)
            } else {
                None
            }
        }
    }
    
    /// Returns the effective sample rate.
    pub fn sample_rate(&self) -> F32Frequency {
        (1.0 / self.time_per_sample).round().try_into().unwrap()
    }

    /// Returns the sample number that an index belongs to. Note that a higher
    /// index and a higher sample number means fresher data.
    ///
    /// This function takes care of the fact that the underlying ringbuffer will
    /// overflow over time and indices will change.
    #[inline]
    fn index_to_sample_number(&self, index: usize) -> usize {
        assert!(index <= self.data().len());
        index + self.lost_samples()
    }

    /// Returns the amount of lost samples, i.e., samples that are no in the
    /// underlying ringbuffer anymore.
    #[inline]
    fn lost_samples(&self) -> usize {
        if self.total_consumed_samples <= self.data().capacity() {
            0
        } else {
            self.total_consumed_samples - self.data().capacity()
        }
    }

    /// Returns the relative timestamp (passed duration) of the given sample,
    /// it is in the range.
    #[inline]
    fn timestamp_of_sample(&self, sample_num: usize) -> Duration {
        if sample_num > self.total_consumed_samples {
            return Duration::default();
        };

        let seconds = sample_num as f32 * self.time_per_sample;
        Duration::from_secs_f32(seconds)
    }

    /// Convenient accessor over [`Self::timestamp_of_sample`] and
    /// [`Self::index_to_sample_number`]
    #[inline]
    fn timestamp_of_index(&self, index: usize) -> Duration {
        let sample_number = self.index_to_sample_number(index);
        self.timestamp_of_sample(sample_number)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio_preprocessing::ValidInputFrequencies;
    use std::iter;

    /// Checks for a sane buffer length with expected effective sample rates of
    /// `<= 500 Hz`.
    ///
    /// You can run `cargo run --bin downsample-bufsize-helper` to get insights on what
    /// we need here.
    #[test]
    fn buffer_len_sane() {
        const SAMPLE_RATE_HZ: f32 = 500.0;

        let sample_rate = 1.0 / SAMPLE_RATE_HZ;
        let duration = Duration::from_secs_f32(sample_rate * DEFAULT_BUFFER_SIZE as f32);
        assert!(duration >= MIN_WINDOW);
        assert!(duration.as_millis() <= 5000);
    }

    #[test]
    fn buffer_memory_footprint_is_sane() {
        // note that for detection, the working set might be much smaller,
        // as the analysis algorithm might only look at the freshest data.
        assert!(size_of::<AudioHistory>() <= 4096);
    }

    #[test]
    fn audio_duration_is_updated_properly() {
        let sample_rate = 2.0.try_into().unwrap();
        let mut hist = AudioHistory::new(sample_rate, None, None);
        assert_eq!(hist.total_consumed_samples, 0);

        hist.update(iter::once(0));
        assert_eq!(hist.total_consumed_samples, 1);
        assert_eq!(hist.passed_time(), Duration::from_secs_f32(0.5));

        hist.update([0, 0].iter().copied());
        assert_eq!(hist.total_consumed_samples, 3);
        assert_eq!(hist.passed_time(), Duration::from_secs_f32(1.5));
    }

    #[test]
    fn index_to_sample_number_works_across_ringbuffer_overflow() {
        let sample_rate = 2.0.try_into().unwrap();
        let mut hist = AudioHistory::new(sample_rate, None, None);

        let test_data = [0; DEFAULT_BUFFER_SIZE + 10];

        hist.update(test_data[0..10].iter().copied());
        assert_eq!(hist.index_to_sample_number(0), 0);
        assert_eq!(hist.index_to_sample_number(10), 10);

        // now the buffer is full, but no overflow yet
        hist.update(test_data[10..DEFAULT_BUFFER_SIZE].iter().copied());
        assert_eq!(hist.index_to_sample_number(0), 0);
        assert_eq!(hist.index_to_sample_number(10), 10);
        assert_eq!(
            hist.index_to_sample_number(DEFAULT_BUFFER_SIZE),
            DEFAULT_BUFFER_SIZE
        );

        // now the buffer overflowed
        hist.update(
            test_data[DEFAULT_BUFFER_SIZE..DEFAULT_BUFFER_SIZE + 10]
                .iter()
                .copied(),
        );
        assert_eq!(hist.index_to_sample_number(0), 10);
        assert_eq!(hist.index_to_sample_number(10), 20);
        assert_eq!(
            hist.index_to_sample_number(DEFAULT_BUFFER_SIZE),
            DEFAULT_BUFFER_SIZE + 10
        );
    }

    #[test]
    // transitively tests timestamp_of_sample()
    fn timestamp_of_index_properly_calculated() {
        let sample_rate = 2.0.try_into().unwrap();
        let mut hist = AudioHistory::new(sample_rate, None, None);

        let test_data = [0; DEFAULT_BUFFER_SIZE + 10];

        hist.update(test_data[0..10].iter().copied());
        assert_eq!(hist.timestamp_of_index(0), Duration::from_secs_f32(0.0));
        assert_eq!(hist.timestamp_of_index(10), Duration::from_secs_f32(5.0));

        // now the buffer is full, but no overflow yet
        hist.update(test_data[10..DEFAULT_BUFFER_SIZE].iter().copied());
        assert_eq!(hist.timestamp_of_index(0), Duration::from_secs_f32(0.0));
        assert_eq!(hist.timestamp_of_index(10), Duration::from_secs_f32(5.0));

        // now the buffer overflowed
        hist.update(
            test_data[DEFAULT_BUFFER_SIZE..DEFAULT_BUFFER_SIZE + 10]
                .iter()
                .copied(),
        );
        assert_eq!(hist.timestamp_of_index(0), Duration::from_secs_f32(5.0));
        assert_eq!(hist.timestamp_of_index(10), Duration::from_secs_f32(10.0));
    }

    #[test]
    fn audio_history_on_real_data() {
        let (samples, header) = crate::test_utils::samples::sample1_long();

        let sample_rate = (header.sample_rate as f32).try_into().unwrap();
        let mut history = AudioHistory::new(sample_rate, None, None);
        history.update(samples.iter().copied());

        assert_eq!(
            (history.passed_time().as_secs_f32() * 1000.0).round() / 1000.0,
            7.999
        );

        let timestamp_at_end = history
            .index_to_sample_info(history.data().capacity() - 1)
            .timestamp
            .as_secs_f32();
        assert_eq!((timestamp_at_end * 1000.0).round() / 1000.0, 7.999);
    }

    #[test]
    fn sample_info() {
        let sample_rate = 1.0.try_into().unwrap();
        let mut hist = AudioHistory::new(sample_rate, None, None);

        hist.update(iter::once(0));
        assert_eq!(
            hist.index_to_sample_info(0).duration_behind,
            Duration::from_secs(0)
        );
        hist.update(iter::once(0));
        assert_eq!(
            hist.index_to_sample_info(0).duration_behind,
            Duration::from_secs(1)
        );
        assert_eq!(
            hist.index_to_sample_info(1).duration_behind,
            Duration::from_secs(0)
        );

        hist.update([0].repeat(hist.data().capacity() * 2).iter().copied());

        assert_eq!(
            hist.index_to_sample_info(0).duration_behind,
            Duration::from_secs_f32((DEFAULT_BUFFER_SIZE - 1) as f32)
        );
        assert_eq!(
            hist.index_to_sample_info(DEFAULT_BUFFER_SIZE - 10)
                .duration_behind,
            Duration::from_secs_f32(9.0)
        );
        assert_eq!(
            hist.index_to_sample_info(DEFAULT_BUFFER_SIZE - 1)
                .duration_behind,
            Duration::from_secs(0)
        );
    }

    /// Ensure that [`SampleInfo`] is ordered by `total_index`.
    #[test]
    fn sample_info_ordering() {
        assert_eq!(
            SampleInfo {
                total_index: 0,
                ..Default::default()
            },
            SampleInfo {
                total_index: 0,
                ..Default::default()
            }
        );

        assert!(
            SampleInfo {
                total_index: 0,
                ..Default::default()
            } < SampleInfo {
                total_index: 1,
                ..Default::default()
            }
        );

        assert!(
            SampleInfo {
                total_index: 11,
                ..Default::default()
            } > SampleInfo {
                total_index: 10,
                ..Default::default()
            }
        );
    }

    #[test]
    fn total_index_to_index_works() {
        let sample_rate = 1.0.try_into().unwrap();
        let mut history = AudioHistory::new(sample_rate, None, None);
        for i in 0..history.data().capacity() {
            assert_eq!(history.total_index_to_index(i), None);
            history.update(iter::once(0));
            assert_eq!(history.total_index_to_index(i), Some(i));
        }

        history.update(iter::once(0));
        // No longer existing.
        assert_eq!(history.total_index_to_index(0), None);
        assert_eq!(history.total_index_to_index(1), Some(0));
        assert_eq!(history.total_index_to_index(2), Some(1));
        assert_eq!(
            history.total_index_to_index(history.total_consumed_samples),
            Some(history.data().capacity())
        );
    }

    #[test]
    fn total_index_original_works() {
        let input = ValidInputFrequencies::new(100.0, 10.0).unwrap();
        let metrics = DownsamplingMetrics::new(input);
        assert_eq!(metrics.factor(), 2);
        assert_eq!(metrics.effective_sample_rate_hz().raw(), 50.0);

        let mut history = AudioHistory::new(input.sample_rate_hz(), Some(metrics), None);

        const N: usize = 10;
        history.update([0; N].into_iter());

        assert_eq!(history.index_to_sample_info(0).total_index_original, 0);
        assert_eq!(history.index_to_sample_info(1).total_index_original, 2);
        assert_eq!(history.index_to_sample_info(9).total_index_original, 18);

        // entirely fill buffer
        for _ in 0..history.audio_buffer.capacity() {
            history.update([0].into_iter());
        }
        assert_eq!(history.total_consumed_samples, DEFAULT_BUFFER_SIZE + N);

        let index = history.audio_buffer.capacity() - 1;
        let expected_index = DEFAULT_BUFFER_SIZE + N - 1;
        assert_eq!(
            history.index_to_sample_info(index).total_index,
            expected_index
        );
        assert_eq!(
            history.index_to_sample_info(index).total_index_original,
            expected_index * 2
        );
        
        // now test with group delay
        history.filter_group_delay.replace(5);
        assert_eq!(
            history.index_to_sample_info(index).total_index_original,
            expected_index * 2 - 5
        );
    }
}
