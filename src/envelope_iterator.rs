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
use crate::MaxMinIterator;
use crate::{AudioHistory, SampleInfo};
use core::cmp::Ordering;
use core::time::Duration;
use ringbuffer::RingBuffer;

/// Threshold to ignore noise.
const ENVELOPE_MIN_VALUE: f32 = 0.1;

/// Ratio between the maximum absolute peak and the absolute average, so that
/// we can be sure there is a clear envelope.
const ENVELOPE_MAX_PEAK_TO_AVG_MIN_RATIO: f32 = 2.0;

/// Minimum sane duration of an envelope. This value comes from looking at
/// waveforms of songs. I picked a beat that I considered as fast/short.
pub(crate) const ENVELOPE_MIN_DURATION_MS: u64 = 140;

/// Minimum realistic duration of an envelope. This value is the result of
/// analyzing some waveforms in Audacity. Specifically, this results from an
/// envelope of two beats very close to each other.
const ENVELOPE_MIN_DURATION: Duration = Duration::from_millis(ENVELOPE_MIN_DURATION_MS);

/// Iterates the envelopes of the provided audio history. An envelope is the set
/// of vibrations(? - german: Schwingungen) that characterize a beat. Its
/// waveform looks somehow like this:
/// ```text
///         x
///        x x       x
///       x   x     x x   x
/// -x---x-----x---x---x-x-x----- (and again for next beat)
///   x x       x x     x
///    x         x
/// ```
///
/// The properties to detect an envelope are not based on scientific research,
/// but on a best-effort and common sense from my side.
///
/// This iterator is supposed to be used multiple times on the same audio
/// history object. However, once the audio history was updated, a new iterator
/// must be created.
#[derive(Debug, Clone)]
pub struct EnvelopeIterator<'a> {
    index: usize,
    buffer: &'a AudioHistory,
}

impl<'a> EnvelopeIterator<'a> {
    pub fn new(buffer: &'a AudioHistory, begin_index: Option<usize>) -> Self {
        let index = begin_index.unwrap_or(0);
        assert!(index < buffer.data().len());
        Self { buffer, index }
    }
}

impl Iterator for EnvelopeIterator<'_> {
    type Item = EnvelopeInfo;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        debug_assert!(self.index < self.buffer.data().len());
        if self.index == self.buffer.data().len() - 1 {
            return None;
        }

        // #####################################################################
        // PREREQUISITES

        // Skip noise.
        let envelope_begin = MaxMinIterator::new(self.buffer, Some(self.index))
            // Find the first item that is not noise.
            .find(|info| info.value_abs >= ENVELOPE_MIN_VALUE)?;

        // Update index to prevent unnecessary iterations on next
        // invocation.
        self.index = envelope_begin.index + 1;

        // First check. Is the (possible) envelope begin far enough behind to
        // actually point to an
        if envelope_begin.duration_behind <= ENVELOPE_MIN_DURATION {
            return None;
        }

        // #####################################################################
        // FIND ENVELOPE

        // Find average.
        let all_peaks_iter =
            MaxMinIterator::new(self.buffer, None /* avg calc over whole history */);
        let peaks_count = all_peaks_iter.clone().count() as f32;
        let peaks_sum = all_peaks_iter
            .map(|info| info.value_abs)
            .reduce(|a, b| a + b)?;
        let peaks_avg = peaks_sum / peaks_count;

        // Sanity checks.
        debug_assert!(peaks_avg > 0.0);
        debug_assert!(peaks_avg <= 1.0);

        // Find max of envelope.
        let envelope_max = MaxMinIterator::new(self.buffer, Some(envelope_begin.index + 1))
            // ignore irrelevant peaks
            .skip_while(|info| info.value_abs / peaks_avg < ENVELOPE_MAX_PEAK_TO_AVG_MIN_RATIO)
            // look at interesting peaks
            .take_while(|info| info.value_abs / peaks_avg >= ENVELOPE_MAX_PEAK_TO_AVG_MIN_RATIO)
            // get the maximum
            .reduce(|a, b| if a.value_abs > b.value_abs { a } else { b })?;

        // Find end of envelope.
        let envelope_end = find_descending_peak_trend_end(self.buffer, envelope_max.index)?;

        // #####################################################################
        // FINALIZE

        let envelope = EnvelopeInfo {
            from: envelope_begin,
            to: envelope_end,
            max: envelope_max,
        };

        // TODO do I need this?
        /*if envelope.duration() < ENVELOPE_MIN_DURATION {
            return None;
        }*/

        // Update index to prevent unnecessary iterations on next
        // invocation.
        self.index = envelope_end.index + 1;

        Some(envelope)
    }
}

/// Helper to find the end of an envelope.
/// Finds the end of an envelope. This itself turned out as complex enough to
/// justify a dedicated, testable function. An envelope ends when the trend of
/// descending (abs) peaks is over. We must prevent that the envelope end
/// clashes with the beginning of the possibly next envelope.
fn find_descending_peak_trend_end(buffer: &AudioHistory, begin_index: usize) -> Option<SampleInfo> {
    assert!(begin_index < buffer.data().len());

    // We allow one peak to be out of line within a trend of descending peaks.
    // But only within this reasonable limit.
    const MAX_NEXT_TO_CURR_OUT_OF_LINE_FACTOR: f32 = 1.05;

    let peak_iter = MaxMinIterator::new(buffer, Some(begin_index));
    peak_iter
        .clone()
        .zip(peak_iter.clone().skip(1).zip(peak_iter.skip(2)))
        .take_while(|(current, (next, nextnext))| {
            let val_curr = current.value_abs;
            let val_next = next.value_abs;
            let val_nextnext = nextnext.value_abs;

            let next_is_descending = val_next <= val_curr;
            if next_is_descending {
                return true;
            }

            let next_to_current_factor = val_next / val_curr;
            debug_assert!(next_to_current_factor > 1.0);

            // nextnext continues descending trend
            next_to_current_factor <= MAX_NEXT_TO_CURR_OUT_OF_LINE_FACTOR
                && val_nextnext <= val_curr
        })
        .last()
        .map(|(current, _)| current)
}

/// Information about an envelope.
#[derive(Clone, Copy, Debug, Default, Eq)]
pub struct EnvelopeInfo {
    pub from: SampleInfo,
    pub to: SampleInfo,
    pub max: SampleInfo,
}

impl EnvelopeInfo {
    /// Returns true if two envelops overlap. This covers the following
    /// scenarios:
    /// ```text
    /// Overlap 1:
    /// |___|      or  |______|
    ///   |___|          |__|
    ///
    /// Overlap 2:
    ///   |___|          |__|
    /// |___|      or  |______|
    ///
    /// Overlap 3:
    /// |___|
    /// |___|
    ///
    /// No Overlap 1:
    ///       |___|
    /// |___|
    ///
    /// No overlap 2:
    /// |___|
    ///       |___|
    /// ```
    pub const fn overlap(&self, other: &Self) -> bool {
        let self_from = self.from.total_index;
        let self_to = self.to.total_index;
        let other_from = other.from.total_index;
        let other_to = other.to.total_index;

        if other_from >= self_from {
            other_from < self_to
        } else if other_from < self_from {
            other_to > self_from
        } else {
            false
        }
    }

    /// The duration/length of the envelope.
    pub fn duration(&self) -> Duration {
        self.to.timestamp - self.from.timestamp
    }

    /// The relative timestamp of the beat/the envelope since the beginning of
    /// the audio recording.
    pub const fn timestamp(&self) -> Duration {
        self.max.timestamp
    }
}

impl PartialOrd for EnvelopeInfo {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EnvelopeInfo {
    fn cmp(&self, other: &Self) -> Ordering {
        self.from
            .partial_cmp(&other.from)
            .expect("Only valid f32 should be here.")
    }
}

impl PartialEq for EnvelopeInfo {
    fn eq(&self, other: &Self) -> bool {
        self.overlap(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils;
    use std::vec::Vec;

    #[test]
    fn envelope_info_overlap() {
        let mut this = EnvelopeInfo::default();
        let mut that = EnvelopeInfo::default();

        this.from.total_index = 0;
        this.to.total_index = 10;

        that.from.total_index = 11;
        that.to.total_index = 20;

        assert!(this.overlap(&this));
        assert!(that.overlap(&that));

        assert!(!this.overlap(&that));
        assert!(!that.overlap(&this));

        that.from.total_index = 10;
        assert!(!this.overlap(&that));
        assert!(!that.overlap(&this));

        that.from.total_index = 9;
        assert!(this.overlap(&that));
        assert!(that.overlap(&this));

        this.from.total_index = 10;
        this.to.total_index = 20;
        that.from.total_index = 10;
        that.to.total_index = 20;
        assert!(this.overlap(&that));
        assert!(that.overlap(&this));

        this.to.total_index = 16;
        assert!(this.overlap(&that));
        assert!(that.overlap(&this));

        this.from.total_index = 10;
        this.to.total_index = 20;
        that.from.total_index = 0;
        that.to.total_index = 10;
        assert!(!this.overlap(&that));
        assert!(!that.overlap(&this));

        this.from.total_index = 10;
        this.to.total_index = 20;
        that.from.total_index = 5;
        that.to.total_index = 15;
        assert!(this.overlap(&that));
        assert!(that.overlap(&this));
    }

    #[test]
    fn find_descending_peak_trend_end_is_correct() {
        // sample1: single beat
        {
            let (samples, header) = test_utils::samples::sample1_single_beat();
            let mut history = AudioHistory::new(header.sampling_rate as f32);
            history.update(samples.iter().copied());

            // Taken from waveform in Audacity.
            let peak_sample_index = 1430;
            assert_eq!(
                find_descending_peak_trend_end(&history, peak_sample_index).map(|info| info.index),
                Some(7099)
            )
        }
        // sample1: double beat
        {
            let (samples, header) = test_utils::samples::sample1_double_beat();
            let mut history = AudioHistory::new(header.sampling_rate as f32);
            history.update(samples.iter().copied());

            // Taken from waveform in Audacity.
            let peak_sample_index = 1634;
            assert_eq!(
                find_descending_peak_trend_end(&history, peak_sample_index).map(|info| info.index),
                Some(6983)
            );

            let peak_sample_index = 8961;
            assert_eq!(
                find_descending_peak_trend_end(&history, peak_sample_index).map(|info| info.index),
                Some(16140)
            );
        }
        // holiday: single beat
        // TODO: Here I discovered that it is not enough to just look at the
        // current, next, and nextnext peak to detect a clear trend. Real music
        // is more complex. But for now, I stick to this approach. I think it is
        // good enough.
        {
            let (samples, header) = test_utils::samples::holiday_single_beat();
            let mut history = AudioHistory::new(header.sampling_rate as f32);
            history.update(samples.iter().copied());

            // Taken from waveform in Audacity.
            let peak_sample_index = 820;
            assert_eq!(
                find_descending_peak_trend_end(&history, peak_sample_index).map(|info| info.index),
                Some(1969)
            )
        }
    }

    #[test]
    fn find_envelopes_sample1_single_beat() {
        let (samples, header) = test_utils::samples::sample1_single_beat();
        let mut history = AudioHistory::new(header.sampling_rate as f32);
        history.update(samples.iter().copied());

        let envelopes = EnvelopeIterator::new(&history, None)
            .take(1)
            .map(|info| (info.from.index, info.to.index))
            .collect::<Vec<_>>();
        assert_eq!(&envelopes, &[(409, 7098)])
    }

    #[test]
    fn find_envelopes_sample1_double_beat() {
        let (samples, header) = test_utils::samples::sample1_double_beat();
        let mut history = AudioHistory::new(header.sampling_rate as f32);
        history.update(samples.iter().copied());

        let envelopes = EnvelopeIterator::new(&history, None)
            .map(|info| (info.from.index, info.to.index))
            .collect::<Vec<_>>();
        #[rustfmt::skip]
        assert_eq!(
            &envelopes,
            &[
                (449, 6978),
                (7328, 16147)
            ]
        );
    }

    #[test]
    fn find_envelopes_holiday_single_beat() {
        let (samples, header) = test_utils::samples::holiday_single_beat();
        let mut history = AudioHistory::new(header.sampling_rate as f32);
        history.update(samples.iter().copied());

        let envelopes = EnvelopeIterator::new(&history, None)
            .map(|info| (info.from.index, info.to.index))
            .collect::<Vec<_>>();
        assert_eq!(&envelopes, &[(259, 1968)]);
    }
}
