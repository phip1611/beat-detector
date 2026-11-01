// SPDX-License-Identifier: MIT

//! Streaming beat detection. See [`BeatDetector`].

use core::time::Duration;
use lowpass_filter::LowpassFilter;

/// Number of samples analyzed at once.
///
/// Small enough to add little latency (1.5 ms at 44.1 kHz), large enough for
/// efficient slice-based filtering.
const BLOCK_LEN: usize = 64;

/// How fast the envelope falls after a peak. Long enough to bridge the zero
/// crossings of a bass wave, short enough to follow the decay of a kick.
const ENVELOPE_RELEASE: Duration = Duration::from_millis(20);

/// Time window of the background level.
const BACKGROUND_WINDOW: Duration = Duration::from_millis(200);

/// Number of blocks over which the rise of the envelope is measured (6 ms at
/// 44.1 kHz). Kicks rise within a few milliseconds, whereas bass notes and
/// other sounds swell in more slowly.
const RISE_BLOCKS: usize = 4;

/// How long the strength of a beat is remembered for
/// [`Config::min_relative_strength`].
const BEAT_MEMORY: Duration = Duration::from_millis(500);

/// Tuning parameters of the [`BeatDetector`].
///
/// The defaults work for typical music. All levels refer to samples in range
/// `-1.0..=1.0`, but only [`Self::min_level`] is absolute: the detector
/// adapts to the volume of the input.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Config {
    /// Frequencies above this are removed before the analysis. Kick drums
    /// have most of their energy below it.
    pub cutoff_hz: f32,
    /// A beat must rise above the background level by this factor. Higher
    /// values mean fewer false positives but more missed beats.
    pub trigger_ratio: f32,
    /// A beat must rise at least to this fraction of the recent beats. This
    /// rejects sounds that are clearly weaker than the beats of the song.
    pub min_relative_strength: f32,
    /// Rises below this level are ignored, e.g., noise.
    pub min_level: f32,
    /// Minimum time between two beats. Limits the maximum tempo.
    pub min_beat_gap: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cutoff_hz: 120.0,
            trigger_ratio: 1.2,
            min_relative_strength: 0.6,
            min_level: 0.005,
            min_beat_gap: Duration::from_millis(100),
        }
    }
}

/// A detected beat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Beat {
    /// Index of the sample at which the beat was detected, counted from the
    /// first sample passed to the detector.
    ///
    /// This is a few milliseconds after the onset of the beat: the time the
    /// detector needs to recognize it.
    pub index: u64,
    /// Time of [`Self::index`] since the first sample.
    pub time: Duration,
}

/// Detects beats in a stream of mono audio samples.
///
/// The detector looks for sudden rises in the level of the bass. It keeps no
/// audio history: its state is a few hundred bytes, and each sample is
/// processed exactly once.
///
/// It works the same for live audio and for whole recordings:
///
/// - **Live**: call [`Self::process`] with each new buffer from the audio
///   input.
/// - **Recordings**: use [`detect_all`].
///
/// The results are independent of how the input is split into buffers.
///
/// # Algorithm
///
/// For each block of 64 samples:
///
/// 1. Remove everything but the bass (below [`Config::cutoff_hz`]).
/// 2. Follow the peak level of the block with an envelope that rises
///    instantly and falls within ~20 ms.
/// 3. Track the background level: the average envelope of the last ~200 ms.
/// 4. Report a beat if the envelope rose within the last ~6 ms by more than
///    - the background level times [`Config::trigger_ratio`],
///    - the strength of the recent beats times
///      [`Config::min_relative_strength`], and
///    - [`Config::min_level`].
///
/// After a beat, the detector waits until the envelope fell below the
/// background level and [`Config::min_beat_gap`] passed.
#[derive(Debug, Clone)]
pub struct BeatDetector {
    config: Config,
    sample_rate_hz: f32,
    lowpass: LowpassFilter<f32>,
    /// Per-block factors derived from the time constants.
    envelope_decay: f32,
    background_weight: f32,
    beat_strength_decay: f32,
    min_beat_gap: u64,

    /// Samples not yet analyzed as they don't fill a block.
    block: [f32; BLOCK_LEN],
    block_fill: usize,
    /// Number of samples analyzed so far.
    position: u64,
    envelope: f32,
    /// The envelopes of the last [`RISE_BLOCKS`] blocks, as ring buffer.
    recent_envelopes: [f32; RISE_BLOCKS],
    recent_index: usize,
    /// `None` until the first block initializes it.
    background: Option<f32>,
    /// Decaying envelope peak of the recent beats.
    beat_strength: f32,
    armed: bool,
    last_beat: Option<u64>,
}

impl BeatDetector {
    /// Creates a detector with the default [`Config`].
    ///
    /// # Panics
    /// Panics if the sample rate is below twice the cutoff frequency, i.e.,
    /// 240 Hz.
    pub fn new(sample_rate_hz: f32) -> Self {
        Self::with_config(sample_rate_hz, Config::default())
    }

    /// Creates a detector with a custom [`Config`].
    ///
    /// # Panics
    /// Panics if the sample rate is below twice [`Config::cutoff_hz`].
    pub fn with_config(sample_rate_hz: f32, config: Config) -> Self {
        let block_secs = BLOCK_LEN as f32 / sample_rate_hz;
        // Approximates exp(-block_secs / time), which isn't available in
        // no_std. Close enough as blocks are much shorter than the time
        // constants.
        let decay = |time: Duration| 1.0 - block_secs / time.as_secs_f32();
        Self {
            config,
            sample_rate_hz,
            lowpass: LowpassFilter::new(sample_rate_hz, config.cutoff_hz),
            envelope_decay: decay(ENVELOPE_RELEASE),
            background_weight: block_secs / BACKGROUND_WINDOW.as_secs_f32(),
            beat_strength_decay: decay(BEAT_MEMORY),
            min_beat_gap: (config.min_beat_gap.as_secs_f32() * sample_rate_hz) as u64,
            block: [0.0; BLOCK_LEN],
            block_fill: 0,
            position: 0,
            envelope: 0.0,
            recent_envelopes: [0.0; RISE_BLOCKS],
            recent_index: 0,
            background: None,
            beat_strength: 0.0,
            armed: true,
            last_beat: None,
        }
    }

    /// Analyzes the next samples of the stream and returns a beat if one was
    /// detected.
    ///
    /// Pass at most [`Self::max_chunk_len`] samples per call; otherwise, a
    /// second beat in the same call would be lost. Live audio buffers are
    /// typically 5-50 ms, way below that.
    pub fn process(&mut self, samples: &[f32]) -> Option<Beat> {
        debug_assert!(
            samples.len() <= self.max_chunk_len(),
            "should pass at most max_chunk_len() samples to not lose beats"
        );
        let mut beat = None;
        let mut samples = samples;
        while !samples.is_empty() {
            let len = (BLOCK_LEN - self.block_fill).min(samples.len());
            let (head, tail) = samples.split_at(len);
            self.block[self.block_fill..][..len].copy_from_slice(head);
            self.block_fill += len;
            samples = tail;

            if self.block_fill == BLOCK_LEN {
                self.block_fill = 0;
                if let Some(found) = self.process_block() {
                    beat = Some(found);
                }
            }
        }
        beat
    }

    /// The maximum number of samples [`Self::process`] accepts per call: the
    /// samples of [`Config::min_beat_gap`]. Such a chunk can't contain two
    /// beats.
    pub fn max_chunk_len(&self) -> usize {
        self.min_beat_gap.max(1) as usize
    }

    /// Current envelope level. For debugging and visualization.
    pub const fn envelope(&self) -> f32 {
        self.envelope
    }

    /// The rise of the envelope required for a beat. For debugging and
    /// visualization.
    pub fn threshold(&self) -> f32 {
        let background = self.background.unwrap_or(0.0);
        (background * self.config.trigger_ratio)
            .max(self.beat_strength * self.config.min_relative_strength)
            .max(self.config.min_level)
    }

    fn process_block(&mut self) -> Option<Beat> {
        self.lowpass.run_slice(&mut self.block);

        let peak = self.block.iter().fold(0.0_f32, |max, s| max.max(s.abs()));
        self.envelope = peak.max(self.envelope * self.envelope_decay);

        let rise = self.envelope - self.recent_envelopes[self.recent_index];
        self.recent_envelopes[self.recent_index] = self.envelope;
        self.recent_index = (self.recent_index + 1) % RISE_BLOCKS;

        // Starting from the first level instead of zero prevents a burst of
        // beats while the background adapts.
        let background = *self.background.get_or_insert(self.envelope);

        self.position += BLOCK_LEN as u64;
        let index = self.position - 1;
        let gap_passed = self
            .last_beat
            .is_none_or(|last| index - last >= self.min_beat_gap);

        let beat = if self.armed && gap_passed && rise > self.threshold() {
            self.armed = false;
            self.last_beat = Some(index);
            Some(Beat {
                index,
                time: Duration::from_secs_f64(index as f64 / f64::from(self.sample_rate_hz)),
            })
        } else {
            if !self.armed {
                // The envelope keeps rising for a few blocks after the beat
                // was detected.
                self.beat_strength = self.beat_strength.max(self.envelope);
            }
            // Re-arming at a lower level prevents that a fading beat that
            // wobbles around the threshold triggers again.
            if self.envelope < background {
                self.armed = true;
            }
            None
        };

        self.background = Some(background + (self.envelope - background) * self.background_weight);
        self.beat_strength *= self.beat_strength_decay;
        beat
    }
}

/// Detects all beats in a complete recording, such as a WAV file.
///
/// The timestamps refer to the beginning of `samples`, so they can be
/// compared with what an audio editor like Audacity shows.
pub fn detect_all(samples: &[f32], sample_rate_hz: f32) -> impl Iterator<Item = Beat> + '_ {
    let mut detector = BeatDetector::new(sample_rate_hz);
    let chunk_len = detector.max_chunk_len();
    samples
        .chunks(chunk_len)
        .filter_map(move |chunk| detector.process(chunk))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::debug::assert_perfect;
    use crate::test_utils::eval::evaluate;
    use crate::test_utils::synth::{Signal, Synth};
    use std::format;
    use std::vec::Vec;

    /// Typical buffer length of a live audio input (5.8 ms at 44.1 kHz).
    const LIVE_CHUNK_LEN: usize = 256;

    /// Feeds the samples like a live input. Returns the detections and the
    /// envelope per sample for the debug views.
    fn run(samples: &[f32], sample_rate: f32, chunk_len: usize) -> (Vec<usize>, Vec<f32>) {
        let mut detector = BeatDetector::new(sample_rate);
        let mut detections = Vec::new();
        let mut envelope = Vec::with_capacity(samples.len());
        for chunk in samples.chunks(chunk_len) {
            if let Some(beat) = detector.process(chunk) {
                detections.push(beat.index as usize);
            }
            envelope.extend(core::iter::repeat_n(detector.envelope(), chunk.len()));
        }
        (detections, envelope)
    }

    /// Asserts that all beats and nothing else are detected, at different
    /// input volumes, as the volume depends on the audio interface.
    fn check(name: &str, signal: Signal) {
        for gain in [1.0, 0.5, 0.1] {
            let signal = Signal {
                samples: signal.samples.iter().map(|s| s * gain).collect(),
                ..signal.clone()
            };
            let (detections, envelope) = run(&signal.samples, signal.sample_rate, LIVE_CHUNK_LEN);
            let name = format!("{name}-gain{gain}");
            assert_perfect(&name, &signal, &detections, &envelope);
        }
    }

    #[test]
    fn kicks() {
        check("kicks", Synth::new(4.0).kicks(120.0, 0.5, 0.8).build());
    }

    #[test]
    fn silence_and_noise() {
        check("silence", Synth::new(2.0).build());
        check("noise", Synth::new(2.0).noise(0.05).build());
    }

    #[test]
    fn double_kicks() {
        let synth = [0.5, 1.5, 2.5].into_iter().fold(Synth::new(3.5), |s, at| {
            s.kick(at, 0.8).kick(at + 0.15, 0.8)
        });
        check("double_kicks", synth.build());
    }

    #[test]
    fn fast_tempo() {
        let synth = Synth::new(4.0)
            .kicks(180.0, 0.5, 0.8)
            .hihats(720.0, 0.5, 0.3);
        check("fast_tempo", synth.build());
    }

    #[test]
    fn kicks_with_hihats_and_snare() {
        let synth = Synth::new(4.0)
            .kicks(120.0, 0.5, 0.8)
            .hihats(480.0, 0.125, 0.4);
        let synth = [1.0, 2.0, 3.0]
            .into_iter()
            .fold(synth, |s, at| s.snare(at, 0.6));
        check("kicks_with_hihats_and_snare", synth.build());
    }

    #[test]
    fn kicks_with_vocal_tone() {
        let synth = Synth::new(4.0).kicks(120.0, 0.5, 0.6).tone(300.0, 0.3);
        check("kicks_with_vocal_tone", synth.build());
    }

    #[test]
    fn loud_then_quieter_kicks() {
        let synth = (0..4).fold(Synth::new(5.0), |s, i| s.kick(0.5 + i as f32 * 0.5, 0.8));
        let synth = (0..4).fold(synth, |s, i| s.kick(2.5 + i as f32 * 0.5, 0.25));
        check("loud_then_quieter_kicks", synth.build());
    }

    #[test]
    fn bass_notes_between_kicks() {
        let synth = Synth::new(4.0).kicks(120.0, 0.5, 0.8);
        let synth = (0..7)
            .map(|i| 0.75 + i as f32 * 0.5)
            .fold(synth, |s, at| s.bass_note(at, 0.2, 55.0, 0.03));
        check("bass_notes_between_kicks", synth.build());
    }

    #[test]
    fn kicks_with_bass_notes() {
        let synth = Synth::new(4.0).kicks(120.0, 0.5, 0.8);
        let synth = (0..7)
            .map(|i| 0.5 + i as f32 * 0.5)
            .fold(synth, |s, at| s.bass_note(at, 0.3, 55.0, 0.03));
        check("kicks_with_bass_notes", synth.build());
    }

    /// Known limitation: after a sudden drop of 18 dB, the first beat is too
    /// weak compared to the remembered strength of the loud beats.
    #[test]
    fn sudden_volume_drop_misses_first_quiet_kick() {
        let synth = (0..4).fold(Synth::new(5.0), |s, i| s.kick(0.5 + i as f32 * 0.5, 0.8));
        let signal = (0..4)
            .fold(synth, |s, i| s.kick(2.5 + i as f32 * 0.5, 0.1))
            .build();
        let (detections, _) = run(&signal.samples, signal.sample_rate, LIVE_CHUNK_LEN);
        let report = evaluate(&signal.beats, &detections, signal.sample_rate);
        assert_eq!(
            (report.missed(), report.false_positives()),
            (1, 0),
            "{report}"
        );
    }

    #[test]
    fn result_is_independent_of_chunk_len() {
        let signal = Synth::new(3.0)
            .kicks(120.0, 0.5, 0.8)
            .hihats(480.0, 0.125, 0.4)
            .build();
        let rate = signal.sample_rate;
        let (expected, _) = run(&signal.samples, rate, LIVE_CHUNK_LEN);
        assert_eq!(expected.len(), signal.beats.len());
        for chunk_len in [1, 7, 64, 1000, 4410] {
            let (detections, _) = run(&signal.samples, rate, chunk_len);
            assert_eq!(detections, expected, "chunk_len={chunk_len}");
        }
    }

    #[test]
    fn detect_all_reports_timestamps_of_whole_recording() {
        let signal = Synth::new(10.0).kicks(120.0, 0.5, 0.8).build();
        let beats = detect_all(&signal.samples, signal.sample_rate).collect::<Vec<_>>();
        assert_eq!(beats.len(), signal.beats.len());
        for (beat, &onset) in beats.iter().zip(&signal.beats) {
            let onset = Duration::from_secs_f32(onset as f32 / signal.sample_rate);
            let latency = beat.time - onset;
            assert!(latency < Duration::from_millis(5), "{latency:?}");
        }
    }

    #[test]
    fn state_is_small() {
        let size = size_of::<BeatDetector>();
        assert!(size <= 512, "{size}");
    }
}
