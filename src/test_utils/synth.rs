// SPDX-License-Identifier: MIT

//! Synthetic test signals with exactly known beats.
//!
//! Real music would need hand-annotated beats, which are imprecise and can't
//! be redistributed freely. Synthetic signals are deterministic, cover
//! specific (adversarial) scenarios, and their ground truth is exact by
//! construction: every [`Synth::kick`] records its onset.
//!
//! The instruments are crude models, just good enough to have the spectral
//! and temporal properties that matter for beat detection.

use core::f32::consts::TAU;
use std::vec::Vec;

/// A generated mono signal and the onsets of its beats.
#[derive(Debug, Clone)]
pub struct Signal {
    /// Samples in range `-1.0..=1.0`.
    pub samples: Vec<f32>,
    pub sample_rate: f32,
    /// Sample indices of the beat onsets, sorted ascending.
    pub beats: Vec<usize>,
}

/// Builder for a [`Signal`]. All times are in seconds.
///
/// ```ignore
/// let signal = Synth::new(4.0).kicks(120.0, 0.5, 0.8).hihats(240.0, 0.25, 0.2).build();
/// ```
#[derive(Debug, Clone)]
pub struct Synth {
    samples: Vec<f32>,
    sample_rate: f32,
    beats: Vec<usize>,
    rng: u32,
}

impl Synth {
    /// Creates silence of the given duration at 44.1 kHz.
    pub fn new(duration: f32) -> Self {
        Self::with_sample_rate(duration, 44100.0)
    }

    pub fn with_sample_rate(duration: f32, sample_rate: f32) -> Self {
        Self {
            samples: vec![0.0; (duration * sample_rate) as usize],
            sample_rate,
            beats: Vec::new(),
            rng: 0x1234_5678,
        }
    }

    /// Adds a kick drum, i.e., a beat: a sine with a fast pitch drop from
    /// 150 Hz to 50 Hz and an exponential decay.
    pub fn kick(mut self, at: f32, amplitude: f32) -> Self {
        let start = self.index(at);
        let sample_rate = self.sample_rate;
        let mut phase = 0.0_f32;
        self.add(start, 0.4, |t| {
            let freq = 50.0 + 100.0 * (-t / 0.02).exp();
            phase += TAU * freq / sample_rate;
            amplitude * (-t / 0.08).exp() * phase.sin()
        });
        self.beats.push(start);
        self
    }

    /// Adds kicks at a regular tempo, from `start` until the end.
    pub fn kicks(self, bpm: f32, start: f32, amplitude: f32) -> Self {
        let times = self.pattern(bpm, start);
        times.fold(self, |synth, at| synth.kick(at, amplitude))
    }

    /// Adds a hi-hat: a short burst of high-frequency noise. Not a beat.
    pub fn hihat(mut self, at: f32, amplitude: f32) -> Self {
        let start = self.index(at);
        let mut rng = self.rng;
        let mut prev = 0.0;
        self.add(start, 0.1, |t| {
            let noise = next_noise(&mut rng);
            // The difference of consecutive samples removes low frequencies.
            let high = noise - prev;
            prev = noise;
            amplitude * 0.5 * (-t / 0.02).exp() * high
        });
        self.rng = rng;
        self
    }

    /// Adds hi-hats at a regular tempo, starting at `offset`.
    pub fn hihats(self, bpm: f32, offset: f32, amplitude: f32) -> Self {
        let times = self.pattern(bpm, offset);
        times.fold(self, |synth, at| synth.hihat(at, amplitude))
    }

    /// Adds a snare drum: a 180 Hz body plus a noise burst. Not a beat, but
    /// its body is low enough to leak through a bass filter.
    pub fn snare(mut self, at: f32, amplitude: f32) -> Self {
        let start = self.index(at);
        let mut rng = self.rng;
        self.add(start, 0.2, |t| {
            let body = (TAU * 180.0 * t).sin() * (-t / 0.04).exp();
            let noise = next_noise(&mut rng) * (-t / 0.06).exp();
            amplitude * (0.6 * body + 0.4 * noise)
        });
        self.rng = rng;
        self
    }

    /// Adds a bass note with a linear attack of `attack` seconds. Not a
    /// beat: in contrast to a kick, it has a pitch and swells in.
    pub fn bass_note(mut self, at: f32, duration: f32, freq: f32, attack: f32) -> Self {
        let start = self.index(at);
        self.add(start, duration, |t| {
            let gain = (t / attack).min(1.0) * ((duration - t) / 0.02).min(1.0);
            0.5 * gain * (TAU * freq * t).sin()
        });
        self
    }

    /// Adds a constant sine over the whole signal, e.g., a held synth pad.
    pub fn tone(mut self, freq: f32, amplitude: f32) -> Self {
        let duration = self.samples.len() as f32 / self.sample_rate;
        self.add(0, duration, |t| amplitude * (TAU * freq * t).sin());
        self
    }

    /// Adds white noise over the whole signal, e.g., a noisy line input.
    pub fn noise(mut self, amplitude: f32) -> Self {
        let mut rng = self.rng;
        let duration = self.samples.len() as f32 / self.sample_rate;
        self.add(0, duration, |_| amplitude * next_noise(&mut rng));
        self.rng = rng;
        self
    }

    /// Adds a constant DC offset, as cheap audio inputs often have.
    pub fn dc_offset(mut self, offset: f32) -> Self {
        self.samples.iter_mut().for_each(|s| *s += offset);
        self
    }

    /// Finalizes the signal and clips it to `-1.0..=1.0`.
    pub fn build(mut self) -> Signal {
        self.samples
            .iter_mut()
            .for_each(|s| *s = s.clamp(-1.0, 1.0));
        self.beats.sort_unstable();
        Signal {
            samples: self.samples,
            sample_rate: self.sample_rate,
            beats: self.beats,
        }
    }

    fn index(&self, at: f32) -> usize {
        (at * self.sample_rate) as usize
    }

    /// Times of a regular pattern from `start` until the end of the signal.
    fn pattern(&self, bpm: f32, start: f32) -> impl Iterator<Item = f32> + use<> {
        let duration = self.samples.len() as f32 / self.sample_rate;
        let period = 60.0 / bpm;
        (0..)
            .map(move |i| start + i as f32 * period)
            .take_while(move |&at| at < duration)
    }

    /// Mixes `f(t)` into the signal for `duration` seconds from `start`,
    /// where `t` is the time since `start`. Truncated at the end.
    fn add(&mut self, start: usize, duration: f32, mut f: impl FnMut(f32) -> f32) {
        let len = (duration * self.sample_rate) as usize;
        let end = (start + len).min(self.samples.len());
        for (i, sample) in self.samples[start..end].iter_mut().enumerate() {
            *sample += f(i as f32 / self.sample_rate);
        }
    }
}

/// Deterministic white noise in range `-1.0..1.0` (xorshift32).
fn next_noise(state: &mut u32) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    *state as f32 / u32::MAX as f32 * 2.0 - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kicks_record_exact_onsets() {
        let signal = Synth::new(2.0).kicks(120.0, 0.25, 0.8).build();
        assert_eq!(signal.sample_rate, 44100.0);
        assert_eq!(signal.beats, [11025, 33075, 55125, 77175]);
    }

    #[test]
    fn signal_is_clipped() {
        let signal = Synth::new(0.5).kick(0.0, 0.9).kick(0.0, 0.9).build();
        let peak = signal.samples.iter().fold(0.0_f32, |m, s| m.max(s.abs()));
        assert_eq!(peak, 1.0);
    }

    #[test]
    fn non_beats_are_not_recorded() {
        let signal = Synth::new(1.0)
            .hihats(240.0, 0.1, 0.5)
            .snare(0.2, 0.5)
            .bass_note(0.3, 0.3, 55.0, 0.05)
            .tone(300.0, 0.1)
            .noise(0.01)
            .dc_offset(0.05)
            .build();
        assert!(signal.beats.is_empty());
    }
}
