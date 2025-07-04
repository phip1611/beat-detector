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
use ringbuffer::RingBuffer;
use crate::audio_preprocessing::audio_history::{AudioHistory, SampleInfo};

const IGNORE_NOISE_THRESHOLD: i16 = (i16::MAX as f32 * 0.05) as i16;

/// The state a sample. Either above x-axis or below.
#[derive(Copy, Clone, PartialEq, Eq)]
enum State {
    /// Above x-axis.
    Above,
    /// Below x-axis.
    Below,
}

impl From<i16 /* sample */> for State {
    #[inline(always)]
    fn from(sample: i16) -> Self {
        if sample.is_positive() {
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
    // This index is updated as we go to reflect the state, i.e., to skip the
    // already processed elements on the next iteration.
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

        // Iter: seeked forward to skip all noise
        let mut iter = self
            .buffer
            .data()
            .iter()
            .enumerate()
            .skip(self.index)
            .skip_while(|(_, &sample)| sample.abs() < IGNORE_NOISE_THRESHOLD);

        let next_element = iter.next().map(|(_, &sample)| sample)?;
        let initial_state = State::from(next_element);

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
    use std::vec::Vec;
    use log::info;
    use crate::audio_preprocessing::conversion::i16_sample_to_f32;
    use crate::audio_preprocessing::lowpass_filter::LowpassFilter;
    use crate::test_utils;

    #[test]
    fn find_roots_in_holiday_excerpt() {
        let (samples, header) = test_utils::samples::holiday_excerpt();
        let sample_rate = header.sample_rate as f32;
        let sample_rate = sample_rate.try_into().unwrap();

        let mut history = AudioHistory::new(sample_rate, None);
        history.update(samples.iter().copied());

        let iter = RootIterator::new(&history, None);
        #[rustfmt::skip]
        assert_eq!(
            iter.map(|info| (info.total_index_original, i16_sample_to_f32(info.amplitude).raw())).collect::<Vec<_>>(),
            // I checked in Audacity whether the values returned by the code
            // make sense. Then, they became the reference for the test.
            [
                (362, -0.0031434065),
                (682, 0.0006408887),
                (923, -0.0020752586),
                (1120, 0.0023499252),
                (1441, -0.00027466659)
            ]
        );
    }

    #[test]
    fn find_roots_in_holiday_excerpt_but_begin_at_specific_index() {
        let (samples, header) = test_utils::samples::holiday_excerpt();
        let sample_rate = header.sample_rate as f32;
        let sample_rate = sample_rate.try_into().unwrap();
        let mut history = AudioHistory::new(sample_rate, None);
        history.update(samples.iter().copied());

        let iter = RootIterator::new(&history, Some(923 /* index taken from test above */ + 1));
        #[rustfmt::skip]
        assert_eq!(
            iter.map(|info| (info.total_index_original, i16_sample_to_f32(info.amplitude).raw())).collect::<Vec<_>>(),
            // I checked in Audacity whether the values returned by the code
            // make sense. Then, they became the reference for the test.
            [
                (1120, 0.0023499252),
                (1441, -0.00027466659)
            ]
        );
    }
    
    #[test]
    fn test_with_downsampler() {/*
        let (samples, header) = test_utils::samples::holiday_excerpt();
        let sample_rate = header.sample_rate as f32;
        let frequencies = ValidInputFrequencies::new(sample_rate, 100.0).unwrap();
        let metrics = DownsamplingMetrics::new(frequencies);
        let lowpass_filter = LowpassFilter::new(frequencies);
        
        let samples = samples.into_iter()
            .map(|sample| lowpass_filter.filter(sample))
        
        let mut history = AudioHistory::new(sample_rate, None);*/
    }
}
