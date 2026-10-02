// SPDX-License-Identifier: MIT

//! Detects all beats in a WAV file, for manual checks against the waveform.
//!
//! ```sh
//! cargo run --release --example analyze-wav -- song.wav
//! ```
//!
//! Prints the beats and writes them as Audacity labels to `song.beats.txt`.
//! In Audacity, open the WAV file and import the labels with
//! "File > Import > Labels..." to see each beat as a marker on the waveform.

use beat_detector::detect_all;
use hound::{SampleFormat, WavReader};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn main() {
    let Some(path) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("Usage: analyze-wav <file.wav>");
        std::process::exit(1);
    };
    let (samples, sample_rate) = read_mono(&path);

    let start = Instant::now();
    let beats = detect_all(&samples, sample_rate).collect::<Vec<_>>();
    let elapsed = start.elapsed();

    println!("   #       time       gap");
    let mut previous = None;
    for (i, beat) in beats.iter().enumerate() {
        let gap = previous.map_or(String::new(), |prev: Duration| {
            format!("{:6.0} ms", (beat.time - prev).as_secs_f64() * 1000.0)
        });
        println!("{:4}  {}  {gap}", i + 1, format_time(beat.time));
        previous = Some(beat.time);
    }

    let duration = samples.len() as f64 / f64::from(sample_rate);
    println!();
    println!("{} beats in {duration:.1} s of audio", beats.len());
    println!(
        "Processing took {elapsed:?}: {:.1} ns per sample, {:.0}x faster than real time",
        elapsed.as_nanos() as f64 / samples.len() as f64,
        duration / elapsed.as_secs_f64()
    );

    let labels_path = path.with_extension("beats.txt");
    let labels = beats.iter().fold(String::new(), |mut labels, beat| {
        let secs = beat.time.as_secs_f64();
        writeln!(labels, "{secs:.6}\t{secs:.6}\tbeat").unwrap();
        labels
    });
    std::fs::write(&labels_path, labels).expect("should be able to write the labels");
    println!("Audacity labels: {}", labels_path.display());
}

/// Reads a WAV file and mixes all channels to mono `f32` samples.
fn read_mono(path: &Path) -> (Vec<f32>, f32) {
    let mut reader = WavReader::open(path).expect("should be a readable WAV file");
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        SampleFormat::Float => reader.samples::<f32>().map(Result::unwrap).collect(),
        SampleFormat::Int => {
            let scale = (1_i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.unwrap() as f32 / scale)
                .collect()
        }
    };
    let channels = usize::from(spec.channels);
    let mono = samples
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect();
    (mono, spec.sample_rate as f32)
}

/// Formats as `mm:ss.mmm`, like the timeline of an audio editor.
fn format_time(time: Duration) -> String {
    let millis = time.as_millis();
    format!(
        "{:02}:{:02}.{:03}",
        millis / 60_000,
        millis / 1000 % 60,
        millis % 1000
    )
}
