use crate::{AudioHistory, SampleInfo};
use ringbuffer::RingBuffer;

const IGNORE_NOISE_THRESHOLD: f32 = 0.05;

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

        let create_iter = || self.buffer.data().iter().enumerate().skip(self.index);

        let next_root = create_iter()
            .zip(create_iter().skip(1))
            .skip_while(|((_, &current), _)| current.abs() < IGNORE_NOISE_THRESHOLD)
            .skip_while(|((_, &current), (_, &next))| {
                // skip while we don't cross the y-axis
                (current < 0.0 && next < 0.0) || (current > 0.0 && next > 0.0)
            })
            .map(|(current, _next)| current)
            .next();

        if let Some((index, _)) = next_root {
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
        history.update(&samples);

        let iter = RootIterator::new(&history, None);
        #[rustfmt::skip]
        assert_eq!(
            iter.map(|info| (info.total_index, info.value)).collect::<Vec<_>>(),
            // I checked in Audacity whether the values returned by the code
            // make sense. Then, they became the reference for the test.
            [
                (362, -0.0031434065),
                (682, 0.00065614795),
                (923, -0.0020905174),
                (1120, 0.002365185),
                (1441, -0.00027466752)
            ]
        );
    }

    /* Unnecessary. No real value-add compared to the basic one.
    #[test]
    fn find_roots_in_sample1_single_beat() {
        let (samples, header) = test_utils::samples::sample1_single_beat();
        let mut history = AudioHistory::new(header.sampling_rate as f32);
        history.update(&samples);

        let iter = RootIterator::new(&history, None);
        #[rustfmt::skip]
        assert_eq!(
            iter.map(|info| (info.total_index, info.value)).collect::<Vec<_>>(),
            // I checked in Audacity whether the values returned by the code
            // make sense. Then, they became the reference for the test.
            [
                (317, 0.0012665181),
                (480, -0.000717185),
                (667, 0.0028382214),
                (930, -0.002929777),
                (1272, 0.0040284432),
                (1604, -0.003051851),
                (1947, 0.0018311106),
                (2305, -0.0016937773),
                (2659, 0.00068666646),
                (3017, -0.0012665181),
                (3363, 0.00012207404),
                (3714, -0.00038148137),
                (4062, 0.0011139256),
                (4416, -0.00065614795),
                (4775, 0.0014496292),
                (5130, -0.0005645924),
                (5479, 0.0010681478),
                (5848, -0.0009155553),
                (6209, 0.0012359996),
                (6571, -0.0012359996),
                (6924, 0.00041199988),
                (7293, -0.0011139256),
                (7661, 0.0003357036),
                (8025, -0.0010681478)
            ]
        );
    }
    */
}
