use beat_detector::*;
use cpal::traits::{DeviceTrait, HostTrait};
use log::LevelFilter;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn init_logger() {
    simple_logger::SimpleLogger::new()
        .with_level(LevelFilter::Trace)
        .with_colors(true)
        .with_utc_timestamps()
        .init()
        .unwrap();
}

fn main() {
    init_logger();
    let stop_recording = Arc::new(AtomicBool::new(false));
    {
        let stop_recording = stop_recording.clone();
        ctrlc::set_handler(move || {
            stop_recording.store(true, Ordering::SeqCst);
        })
        .unwrap();
    }

    log::info!("Supported audio backends");
    for (_backend, host) in get_backends() {
        for device in host.devices().unwrap() {
            log::info!(
                "{} => {}",
                host.id().name(),
                device.name().unwrap_or("<unknown>".to_string())
            );
        }
    }

    let _handle = recording::start_detector_thread(
        |info| {
            println!("beat: {info:?}");
        },
        None,
    )
    .unwrap();

    log::info!("Start recording");
    while !stop_recording.load(Ordering::SeqCst) {}
    log::info!("Stopped recording");
}

fn get_backends() -> HashMap<String, cpal::Host> {
    cpal::available_hosts()
        .into_iter()
        .map(|id| (format!("{:?}", id), cpal::host_from_id(id).unwrap()))
        .collect::<HashMap<_, _>>()
}
