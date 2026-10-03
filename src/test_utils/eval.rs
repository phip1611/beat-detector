// SPDX-License-Identifier: MIT

//! Compares detected beats against the ground truth.
//!
//! The central number is the latency: the time from the beat's onset in the
//! audio input until the detector reported it.

use core::fmt::{self, Display, Formatter};
use std::vec::Vec;

/// A detection is only attributed to an onset if it follows within this
/// time. Anything later is too late to be useful, e.g., for lights.
pub const MAX_LATENCY_MS: f32 = 50.0;

/// One row of a [`Report`]. All values are sample indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Detected { onset: usize, detected: usize },
    Missed { onset: usize },
    FalsePositive { detected: usize },
}

impl Outcome {
    const fn sort_key(&self) -> usize {
        match *self {
            Self::Detected { onset, .. } | Self::Missed { onset } => onset,
            Self::FalsePositive { detected } => detected,
        }
    }
}

/// Result of [`evaluate`]. Its [`Display`] implementation renders a table
/// meant for humans.
#[derive(Debug, Clone)]
pub struct Report {
    pub outcomes: Vec<Outcome>,
    pub sample_rate: f32,
}

/// Matches each detection to the earliest unmatched onset at most
/// [`MAX_LATENCY_MS`] before it. Both inputs are sorted sample indices.
pub fn evaluate(onsets: &[usize], detections: &[usize], sample_rate: f32) -> Report {
    let max_latency = (MAX_LATENCY_MS / 1000.0 * sample_rate) as usize;
    let mut onsets = onsets.iter().copied().peekable();
    let mut outcomes = Vec::new();

    for &detected in detections {
        // Onsets that can't be matched by this or any later detection.
        while let Some(onset) = onsets.next_if(|&onset| onset + max_latency < detected) {
            outcomes.push(Outcome::Missed { onset });
        }
        match onsets.next_if(|&onset| onset <= detected) {
            Some(onset) => outcomes.push(Outcome::Detected { onset, detected }),
            None => outcomes.push(Outcome::FalsePositive { detected }),
        }
    }
    outcomes.extend(onsets.map(|onset| Outcome::Missed { onset }));
    outcomes.sort_by_key(Outcome::sort_key);

    Report {
        outcomes,
        sample_rate,
    }
}

impl Report {
    /// Latencies of all detected beats in milliseconds.
    pub fn latencies_ms(&self) -> impl Iterator<Item = f32> + '_ {
        self.outcomes.iter().filter_map(|outcome| match *outcome {
            Outcome::Detected { onset, detected } => Some(self.ms(detected - onset)),
            _ => None,
        })
    }

    pub fn missed(&self) -> usize {
        self.count(|o| matches!(o, Outcome::Missed { .. }))
    }

    pub fn false_positives(&self) -> usize {
        self.count(|o| matches!(o, Outcome::FalsePositive { .. }))
    }

    pub fn is_perfect(&self) -> bool {
        self.missed() == 0 && self.false_positives() == 0
    }

    fn count(&self, f: impl Fn(&Outcome) -> bool) -> usize {
        self.outcomes.iter().filter(|o| f(o)).count()
    }

    fn ms(&self, samples: usize) -> f32 {
        samples as f32 / self.sample_rate * 1000.0
    }
}

impl Display for Report {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "     onset    detected     latency")?;
        for outcome in &self.outcomes {
            match *outcome {
                Outcome::Detected { onset, detected } => writeln!(
                    f,
                    "{:7.1} ms  {:7.1} ms  {:7.1} ms",
                    self.ms(onset),
                    self.ms(detected),
                    self.ms(detected - onset)
                )?,
                Outcome::Missed { onset } => {
                    writeln!(f, "{:7.1} ms           -  MISSED", self.ms(onset))?
                }
                Outcome::FalsePositive { detected } => writeln!(
                    f,
                    "         -  {:7.1} ms  FALSE POSITIVE",
                    self.ms(detected)
                )?,
            }
        }

        let latencies = self.latencies_ms().collect::<Vec<_>>();
        let detected = latencies.len();
        let max = latencies.iter().copied().fold(0.0, f32::max);
        // Summing an empty float iterator yields -0.0.
        let avg = if detected == 0 {
            0.0
        } else {
            latencies.iter().sum::<f32>() / detected as f32
        };
        write!(
            f,
            "{detected}/{} detected, {} false positives, latency avg {avg:.1} ms, max {max:.1} ms",
            detected + self.missed(),
            self.false_positives(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::string::ToString;

    // 1 sample = 1 ms keeps the numbers readable.
    const RATE: f32 = 1000.0;

    #[test]
    fn matches_detections_after_onsets() {
        let report = evaluate(&[100, 500], &[103, 510], RATE);
        assert_eq!(
            report.outcomes,
            [
                Outcome::Detected {
                    onset: 100,
                    detected: 103
                },
                Outcome::Detected {
                    onset: 500,
                    detected: 510
                },
            ]
        );
        assert!(report.is_perfect());
        assert_eq!(report.latencies_ms().collect::<Vec<_>>(), [3.0, 10.0]);
    }

    #[test]
    fn reports_misses_and_false_positives() {
        // 300 is detected too late, 120 belongs to no onset, 900 is never
        // detected.
        let report = evaluate(&[100, 300, 900], &[101, 120, 351], RATE);
        assert_eq!(
            report.outcomes,
            [
                Outcome::Detected {
                    onset: 100,
                    detected: 101
                },
                Outcome::FalsePositive { detected: 120 },
                Outcome::Missed { onset: 300 },
                Outcome::FalsePositive { detected: 351 },
                Outcome::Missed { onset: 900 },
            ]
        );
        assert_eq!(report.missed(), 2);
        assert_eq!(report.false_positives(), 2);
    }

    #[test]
    fn detection_before_onset_is_false_positive() {
        let report = evaluate(&[100], &[99], RATE);
        assert_eq!(report.false_positives(), 1);
        assert_eq!(report.missed(), 1);
    }

    #[test]
    fn summary_without_detections() {
        let report = evaluate(&[100], &[], RATE);
        assert!(
            report
                .to_string()
                .ends_with("latency avg 0.0 ms, max 0.0 ms")
        );
    }

    #[test]
    fn renders_table() {
        let report = evaluate(&[100, 300], &[103, 200], RATE);
        let expected = "     onset    detected     latency
  100.0 ms    103.0 ms      3.0 ms
         -    200.0 ms  FALSE POSITIVE
  300.0 ms           -  MISSED
1/2 detected, 1 false positives, latency avg 3.0 ms, max 3.0 ms";
        assert_eq!(report.to_string(), expected);
    }
}
