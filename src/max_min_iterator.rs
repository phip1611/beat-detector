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
        let index = RootIterator::new(buffer, Some(index))
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
        let end_index = RootIterator::new(self.buffer, Some(begin_index))
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
            // TODO by increasing this, we also have high performance
            //  improvement chances.
            .step_by(2)
            .max_by(|(_x_index, &x_value), (_y_index, &y_value)| {
                if libm::fabsf(x_value) > libm::fabsf(y_value) {
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
        history.update(samples.iter().copied());

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
                (1301, -0.30671102)
            ]
        );
    }
}
