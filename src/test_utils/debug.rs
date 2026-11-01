// SPDX-License-Identifier: MIT

//! Helpers to look at signals and beats when a test fails.
//!
//! Both views show the rectified signal (`|x|`), as the detector only cares
//! about the level, plus the beat onsets and the detections:
//!
//! - [`ascii_view`]: for the terminal, one column per 10 ms
//! - [`write_png`]: for an image viewer, in `target/test-artifacts/`

use super::eval::evaluate;
use super::synth::Signal;
use super::target_dir_test_artifacts;
use audio_visualizer::WaveformVisualizer;
use std::path::PathBuf;
use std::string::String;
use std::vec::Vec;
use std::{eprintln, format};

const COLUMN_MS: f32 = 10.0;
const COLUMNS_PER_ROW: usize = 100;
const LEVELS: &[u8] = b" .:-=+*#%@";

/// Renders the signal as text. Each column shows the peak level of 10 ms,
/// scaled to the loudest column. The line below marks onsets with `^`,
/// detections with `B`, and both in the same column with `X`.
///
/// ```text
///      0 ms | %#*=-:..         %#*=-:..         %#*=-:..
///           | X                ^B               X
/// ```
pub fn ascii_view(
    signal: &[f32],
    sample_rate: f32,
    onsets: &[usize],
    detections: &[usize],
) -> String {
    let column_len = ((COLUMN_MS / 1000.0 * sample_rate) as usize).max(1);
    let peaks = signal
        .chunks(column_len)
        .map(|chunk| chunk.iter().fold(0.0_f32, |m, s| m.max(s.abs())))
        .collect::<Vec<_>>();
    let max = peaks.iter().copied().fold(f32::EPSILON, f32::max);

    let mut marks = vec![b' '; peaks.len()];
    for &onset in onsets {
        marks[onset / column_len] = b'^';
    }
    for &detected in detections {
        let mark = &mut marks[detected / column_len];
        *mark = if *mark == b' ' { b'B' } else { b'X' };
    }

    let mut out = String::new();
    for (row, (peaks, marks)) in peaks
        .chunks(COLUMNS_PER_ROW)
        .zip(marks.chunks(COLUMNS_PER_ROW))
        .enumerate()
    {
        let levels = peaks
            .iter()
            .map(|peak| {
                let level = (peak / max * (LEVELS.len() - 1) as f32).round() as usize;
                LEVELS[level] as char
            })
            .collect::<String>();
        let ms = row * COLUMNS_PER_ROW * COLUMN_MS as usize;
        out += &format!("{ms:6} ms |{levels}\n");
        out += &format!("          |{}\n", String::from_utf8_lossy(marks));
    }
    out
}

/// Writes the signal as PNG to `target/test-artifacts/<name>.png` and
/// returns the path.
///
/// The upper half shows `|signal|`. The lower half marks onsets with lines
/// down to `-0.5` and detections with lines down to `-1.0`.
pub fn write_png(
    name: &str,
    signal: &[f32],
    sample_rate: f32,
    onsets: &[usize],
    detections: &[usize],
) -> PathBuf {
    let max = signal.iter().fold(f32::EPSILON, |m, s| m.max(s.abs()));
    let mut image = signal.iter().map(|s| s.abs() / max).collect::<Vec<_>>();
    for &onset in onsets {
        image[onset] = -0.5;
    }
    for &detected in detections {
        image[detected] = -1.0;
    }

    let path = target_dir_test_artifacts().join(format!("{name}.png"));
    WaveformVisualizer::new(&image)
        .sample_rate(sample_rate)
        .y_range(-1.0..1.0)
        .title(format!(
            "{name}: |signal| (top), onsets -0.5, detections -1.0 (bottom)"
        ))
        .write_png(&path)
        .expect("should be able to write to the target directory");
    path
}

/// Asserts that the detections match the beats of the signal exactly. On
/// failure, prints the evaluation table and the [`ascii_view`] of `view`,
/// and writes a [`write_png`] image of it.
///
/// `view` is the signal to look at, e.g., the input or the detector's
/// envelope; it must have the length of the input.
pub fn assert_perfect(name: &str, signal: &Signal, detections: &[usize], view: &[f32]) {
    let report = evaluate(&signal.beats, detections, signal.sample_rate);
    if report.is_perfect() {
        return;
    }
    let ascii = ascii_view(view, signal.sample_rate, &signal.beats, detections);
    let png = write_png(name, view, signal.sample_rate, &signal.beats, detections);
    eprintln!("{report}\n\n{ascii}");
    panic!(
        "{name}: detections differ from beats, see {}",
        png.display()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::synth::Synth;

    #[test]
    fn ascii_view_marks_onsets_and_detections() {
        let signal = Synth::new(0.3).kick(0.0, 0.8).kick(0.2, 0.8).build();
        let view = ascii_view(
            &signal.samples,
            signal.sample_rate,
            &signal.beats,
            &[441, 8900],
        );
        let lines = view.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("     0 ms |@"));
        // Kick at 0 ms detected in the next column, kick at 200 ms in its own.
        assert_eq!(&lines[1][11..13], "^B");
        assert_eq!(&lines[1][31..32], "X");
    }

    #[test]
    fn write_png_creates_file() {
        let signal = Synth::new(1.0).kicks(120.0, 0.25, 0.8).build();
        let path = write_png(
            "debug-write-png",
            &signal.samples,
            signal.sample_rate,
            &signal.beats,
            &[11100],
        );
        assert!(path.exists());
    }

    #[test]
    #[should_panic(expected = "detections differ from beats")]
    fn assert_perfect_panics_on_mismatch() {
        let signal = Synth::new(1.0).kicks(120.0, 0.25, 0.8).build();
        assert_perfect("debug-assert-perfect", &signal, &[], &signal.samples);
    }
}
