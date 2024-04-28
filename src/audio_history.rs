use crate::envelope_iterator::ENVELOPE_MIN_DURATION_MS;
use core::cmp::Ordering;
use core::time::Duration;
use ringbuffer::{ConstGenericRingBuffer, RingBuffer};

const SAFETY_BUFFER_FACTOR: f64 = 3.0;
/// Length in ms of the captured audio history used for analysis.
pub(crate) const DEFAULT_AUDIO_HISTORY_WINDOW_MS: usize =
    (ENVELOPE_MIN_DURATION_MS as f64 * SAFETY_BUFFER_FACTOR) as usize;

/// Based on the de-facto default sampling rate of 44100 Hz / 44.1 kHz.
const DEFAULT_SAMPLES_PER_SECOND: usize = 44100;
const MS_PER_SECOND: usize = 1000;

/// Default buffer size for [`AudioHistory`]. The size is a trade-off between
/// memory efficiency and effectiveness in detecting envelops properly.
pub const DEFAULT_BUFFER_SIZE: usize =
    (DEFAULT_AUDIO_HISTORY_WINDOW_MS * DEFAULT_SAMPLES_PER_SECOND) / MS_PER_SECOND;

/// Sample info with time context.
#[derive(Copy, Clone, Debug)]
pub struct SampleInfo {
    /// The value of the sample in range `[-1.0..=1.0]`.
    pub value: f32,
    /// The current index in [`AudioHistory`].
    pub index: usize,
    /// The total index since the beginning of audio history.
    pub total_index: usize,
    /// Relative timestamp since beginning of audio history.
    pub timestamp: Duration,
    /// The time the sample is behind the latest data.
    pub duration_behind: Duration,
}

impl Default for SampleInfo {
    fn default() -> Self {
        Self {
            value: 0.0,
            index: 0,
            total_index: 0,
            timestamp: Default::default(),
            duration_behind: Default::default(),
        }
    }
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
        self.total_index
            .partial_cmp(&other.total_index)
            .expect("Should be comparable")
    }
}

/// Accessor over the captured audio history that helps to identify the
/// timestamp of each sample. Users are supposed to add new data in chunks that
/// are less than the buffer size, to slowly fade out old data from the
/// underlying ringbuffer.
#[derive(Debug)]
pub struct AudioHistory {
    audio_buffer: ConstGenericRingBuffer<f32, DEFAULT_BUFFER_SIZE>,
    total_consumed_items: usize,
    time_per_sample: f32,
}

impl AudioHistory {
    pub fn new(sampling_frequency: f32) -> Self {
        let audio_buffer = ConstGenericRingBuffer::new();
        assert!(sampling_frequency.is_normal() && sampling_frequency.is_sign_positive());
        Self {
            audio_buffer,
            time_per_sample: 1.0 / sampling_frequency,
            total_consumed_items: 0,
        }
    }

    /// Update the audio history with fresh samples. The audio samples are
    /// expected to be in mono channel format.
    pub fn update<I: Iterator<Item = f32>>(&mut self, mono_samples_iter: I) {
        let mut len = 0;
        mono_samples_iter
            .for_each(|sample| {
                debug_assert!(sample.is_finite());
                debug_assert!(sample.abs() <= 1.0);

                self.audio_buffer.push(sample);
                len += 1;
            });

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

        self.total_consumed_items += len;
    }

    /// Get the passed time in seconds.
    pub fn passed_time(&self) -> Duration {
        let seconds = self.time_per_sample * self.total_consumed_items as f32;
        Duration::from_secs_f32(seconds)
    }

    /// Access the underlying data storage.
    pub const fn data(&self) -> &ConstGenericRingBuffer<f32, DEFAULT_BUFFER_SIZE> {
        &self.audio_buffer
    }

    /// Returns the [`SampleInfo`] about a sample from the current index of that
    /// sample.
    #[inline]
    pub fn index_to_sample_info(&self, index: usize) -> SampleInfo {
        assert!(index < self.data().capacity());

        let timestamp = self.timestamp_of_index(index);
        SampleInfo {
            index,
            timestamp,
            value: self.data()[index],
            total_index: self.index_to_sample_number(index),
            duration_behind: self.timestamp_of_index(self.data().len() - 1) - timestamp,
        }
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
    fn lost_samples(&self) -> usize {
        if self.total_consumed_items <= self.data().capacity() {
            0
        } else {
            self.total_consumed_items - self.data().capacity()
        }
    }

    /// Returns the relative timestamp (passed duration) of the given sample,
    /// it is in the range.
    #[inline]
    fn timestamp_of_sample(&self, sample_num: usize) -> Duration {
        if sample_num > self.total_consumed_items {
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

    /*/// Getter for the sampling frequency.
    pub fn sampling_frequency(&self) -> f32 {
        1.0 / self.time_per_sample
    }*/
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    #[test]
    fn buffer_len_sane() {
        let sampling_rate = 1.0 / DEFAULT_SAMPLES_PER_SECOND as f32;
        let duration = Duration::from_secs_f32(sampling_rate * DEFAULT_BUFFER_SIZE as f32);
        dbg!(duration);
        assert!(duration.as_millis() > 10);
        assert!(duration.as_millis() <= 1000);
    }

    #[test]
    fn audio_duration_is_updated_properly() {
        let mut hist = AudioHistory::new(2.0);
        assert_eq!(hist.total_consumed_items, 0);

        hist.update([0.0].iter().copied());
        assert_eq!(hist.total_consumed_items, 1);
        assert_eq!(hist.passed_time(), Duration::from_secs_f32(0.5));

        hist.update([0.0, 0.0].iter().copied());
        assert_eq!(hist.total_consumed_items, 3);
        assert_eq!(hist.passed_time(), Duration::from_secs_f32(1.5));
    }

    #[test]
    fn index_to_sample_number_works_across_ringbuffer_overflow() {
        let mut hist = AudioHistory::new(2.0);

        // buffer capacity + 10 items
        let test_data = (0..DEFAULT_BUFFER_SIZE + 10)
            .map(|x| x as f32 / (DEFAULT_BUFFER_SIZE + 10) as f32)
            .collect::<Vec<_>>();

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
        let mut hist = AudioHistory::new(2.0);

        // buffer capacity + 10 items
        let test_data = (0..DEFAULT_BUFFER_SIZE + 10)
            .map(|x| x as f32 / (DEFAULT_BUFFER_SIZE + 10) as f32)
            .collect::<Vec<_>>();

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

        let mut history = AudioHistory::new(header.sampling_rate as f32);
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
        let mut hist = AudioHistory::new(1.0);

        hist.update([0.0].iter().copied());
        assert_eq!(
            hist.index_to_sample_info(0).duration_behind,
            Duration::from_secs(0)
        );
        hist.update([0.0].iter().copied());
        assert_eq!(
            hist.index_to_sample_info(0).duration_behind,
            Duration::from_secs(1)
        );
        assert_eq!(
            hist.index_to_sample_info(1).duration_behind,
            Duration::from_secs(0)
        );

        hist.update([0.0].repeat(hist.data().capacity() * 2).iter().copied());

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
}
