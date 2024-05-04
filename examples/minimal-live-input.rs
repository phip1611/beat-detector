use beat_detector::recording;
use cpal::traits::StreamTrait;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[path = "_modules/example_utils.rs"]
mod example_utils;

fn main() {
    example_utils::init_logger();
    let input_device = example_utils::select_audio_device();

    let stop_recording = Arc::new(AtomicBool::new(false));
    {
        let stop_recording = stop_recording.clone();
        ctrlc::set_handler(move || {
            stop_recording.store(true, Ordering::SeqCst);
        })
        .unwrap();
    }

    let handle = recording::start_detector_thread(
        |info| {
            println!("beat: {info:?}");
        },
        Some(input_device),
    )
    .unwrap();

    log::info!("Start recording");
    while !stop_recording.load(Ordering::SeqCst) {}
    handle.pause().unwrap();
    log::info!("Stopped recording");
}
