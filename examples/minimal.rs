/*
MIT License

Copyright (c) 2021 Philipp Schuster

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
//! Minimum example on how to use this library. Sets up the "callback loop".

use cpal::Device;
use beat_detector::StrategyKind;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Minimum example on how to use this library. Sets up the "callback loop".
fn main() {
    let recording = Arc::new(AtomicBool::new(true));

    let recording_cpy = recording.clone();
    ctrlc::set_handler(move || {
        eprintln!("Stopping recording");
        recording_cpy.store(false, Ordering::SeqCst);
    }).unwrap();

    let dev = select_input_device();
    let strategy = select_strategy();
    let on_beat = |info| {
        println!("Found beat at {:?}ms", info);
    };
    // actually start listening in thread
    let handle = beat_detector::record::start_listening(
        on_beat,
        Some(dev),
        strategy,
        recording,
    ).unwrap();

    handle.join().unwrap();
}

fn select_input_device() -> Device {
    // todo implement user selection
    beat_detector::record::audio_input_device_list().into_iter().next().expect("At least one audio input device must be available.").1
}

fn select_strategy() -> StrategyKind {
    // todo implement user selection
    StrategyKind::Spectrum
}