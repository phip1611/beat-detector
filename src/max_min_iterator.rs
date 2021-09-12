use crate::RootIterator;
use crate::{AudioHistory, SampleInfo};
use core::cmp::Ordering;
use ringbuffer::RingBuffer;

// const IGNORE_NOISE_THRESHOLD: f32 = 0.05;

/// Iterates the minima and maxima of the wave.
///
/// This iterator is supposed to be used multiple times on the same audio
/// history object. However, once the audio history was updated, a new iterator
/// must be created.
#[derive(Debug, Clone)]
pub struct MaxMinIterator<'a> {
    index: usize,
    buffer: &'a AudioHistory,
}

impl<'a> MaxMinIterator<'a> {
    /// Creates a new iterator. Immediately moves the index to point to the
    /// next root of the wave. This way, we prevent detection of
    /// "invalid/false peaks" before the first root has been found.
    pub fn new(buffer: &'a AudioHistory, begin_index: Option<usize>) -> Self {
        let index = begin_index.unwrap_or(0);
        assert!(index < buffer.data().len());
        let index = RootIterator::new(&buffer, Some(index))
            .next()
            .map(|info| info.index)
            .unwrap_or(buffer.data().len() - 1);
        Self { buffer, index }
    }
}

impl Iterator for MaxMinIterator<'_> {
    type Item = SampleInfo;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        debug_assert!(self.index < self.buffer.data().len());
        if self.index == self.buffer.data().len() - 1 {
            return None;
        }

        let begin_index = self.index;
        let end_index = RootIterator::new(&self.buffer, Some(begin_index))
            .next()?
            .index;
        let sample_count = end_index - begin_index;

        let max_or_min = self
            .buffer
            .data()
            .iter()
            .enumerate()
            .skip(begin_index)
            .take(sample_count)
            .max_by(|(_x_index, &x_value), (_y_index, &y_value)| {
                if x_value.abs() > y_value.abs() {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }
            });

        max_or_min.map(|(index, _max_or_min)| {
            // + 1: don't find the same the next time
            self.index = end_index + 1;
            self.buffer.index_to_sample_info(index)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils;
    use std::vec::Vec;

    #[test]
    fn find_maxmin_in_holiday_excerpt() {
        let (samples, header) = test_utils::samples::holiday_excerpt();
        let mut history = AudioHistory::new(header.sampling_rate as f32);
        history.update(&samples);

        let iter = MaxMinIterator::new(&history, None);
        #[rustfmt::skip]
        assert_eq!(
            iter.map(|info| (info.total_index, info.value)).collect::<Vec<_>>(),
            // I checked in Audacity whether the values returned by the code
            // make sense. Then, they became the reference for the test.
            [
                (543, 0.39106417),
                (865, -0.068865016),
                (1027, 0.24600971),
                (1302, -0.3068636)
            ]
        );
    }

    /* Unnecessary. No real value-add compared to the basic one.
    #[test]
    fn find_maxmin_in_sample1_single_beat() {
        let (samples, header) = test_utils::samples::sample1_single_beat();
        let mut history = AudioHistory::new(header.sampling_rate as f32);
        history.update(&samples);

        let iter = MaxMinIterator::new(&history, None);
        #[rustfmt::skip]
        assert_eq!(
            iter.map(|info| (info.total_index, info.value)).collect::<Vec<_>>(),
            // I checked in Audacity whether the values returned by the code
            // make sense. Then, they became the reference for the test.
            [
                (278, 0.052491836),
                (410, -0.16049685),
                (571, 0.373455),
                (784, -0.5160222),
                (1115, 0.5157323),
                (1430, -0.6508072),
                (1765, 0.57049775),
                (2134, -0.43621632),
                (2468, 0.33156836),
                (2835, -0.25760674),
                (3182, 0.24184392),
                (3524, -0.23711357),
                (3873, 0.24263741),
                (4238, -0.2305063),
                (4594, 0.22055727),
                (4949, -0.21684927),
                (5308, 0.20571002),
                (5688, -0.17737357),
                (6029, 0.1653035),
                (6397, -0.16501358),
                (6757, 0.16016114),
                (7098, -0.14281136),
                (7462, 0.14355907),
                (7840, -0.1374096),
            ]
        );
    } */
}
