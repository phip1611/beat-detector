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
use crate::{AudioHistory, SampleInfo};
use ringbuffer::RingBuffer;

const IGNORE_NOISE_THRESHOLD: f32 = 0.05;

/// THe state a sample. Either above zero or below.
#[derive(Copy, Clone, PartialEq, Eq)]
enum State {
    Above,
    Below,
}

impl From<f32 /* sample */> for State {
    #[inline(always)]
    fn from(sample: f32) -> Self {
        debug_assert!(libm::fabsf(sample) <= 1.0);
        if sample.is_sign_positive() {
            Self::Above
        } else {
            Self::Below
        }
    }
}

/// Iterates the roots/zeroes of the wave.
///
/// This iterator is supposed to be used multiple times on the same audio
/// history object. However, once the audio history was updated, a new iterator
/// must be created.
#[derive(Debug, Clone)]
pub struct RootIterator<'a> {
    index: usize,
    buffer: &'a AudioHistory,
}

impl<'a> RootIterator<'a> {
    pub fn new(buffer: &'a AudioHistory, begin_index: Option<usize>) -> Self {
        let index = begin_index.unwrap_or(0);
        assert!(index < buffer.data().len());
        Self { buffer, index }
    }
}

impl Iterator for RootIterator<'_> {
    type Item = SampleInfo;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        debug_assert!(self.index < self.buffer.data().len());
        if self.index == self.buffer.data().len() - 1 {
            return None;
        }

        let mut iter = self
            .buffer
            .data()
            .iter()
            .enumerate()
            .skip(self.index)
            // Given the very high sampling rate, we can sacrifice a negligible
            // impact on precision for better performance / fewer iterations.
            .step_by(2)
            .skip_while(|(_, &sample)| libm::fabsf(sample) < IGNORE_NOISE_THRESHOLD);

        let initial_state = State::from(iter.next().map(|(_, &sample)| sample)?);

        let next_root = iter
            // Skip while we didn't cross the x axis.
            .find(|(_, &sample)| State::from(sample) != initial_state)
            // We are looking for the index right before the zero.
            .map(|(index, _)| index - 1);

        if let Some(index) = next_root {
            // + 1: don't find the same the next time
            self.index = index + 1;
            Some(self.buffer.index_to_sample_info(index))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils;
    use std::vec::Vec;

    #[test]
    fn find_roots_in_holiday_excerpt() {
        let (samples, header) = test_utils::samples::holiday_excerpt();
        let mut history = AudioHistory::new(header.sampling_rate as f32);
        history.update(samples.iter().copied());

        let iter = RootIterator::new(&history, None);
        #[rustfmt::skip]
        assert_eq!(
            iter.map(|info| (info.total_index, info.value)).collect::<Vec<_>>(),
            // I checked in Audacity whether the values returned by the code
            // make sense. Then, they became the reference for the test.
            [
                (363, 0.0017242958),
                (683, -0.0015106662),
                (923, -0.0020905174),
                (1121, -0.0013580737),
                (1441, -0.00027466752)
            ]
        );
    }

    #[test]
    fn find_roots_in_holiday_excerpt_but_begin_at_specific_index() {
        let (samples, header) = test_utils::samples::holiday_excerpt();
        let mut history = AudioHistory::new(header.sampling_rate as f32);
        history.update(samples.iter().copied());

        let iter = RootIterator::new(&history, Some(923 /* index taken from test above */ + 1));
        #[rustfmt::skip]
        assert_eq!(
            iter.map(|info| (info.total_index, info.value)).collect::<Vec<_>>(),
            // I checked in Audacity whether the values returned by the code
            // make sense. Then, they became the reference for the test.
            [
                (1121, -0.0013580737),
                (1441, -0.00027466752)
            ]
        );
    }
}
