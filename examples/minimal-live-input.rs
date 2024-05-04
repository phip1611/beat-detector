use beat_detector::*;
use cpal::traits::{DeviceTrait, HostTrait};
use log::LevelFilter;
use std::io::Read;
use std::process::exit;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn init_logger() {
    simple_logger::SimpleLogger::new()
        .with_level(LevelFilter::Debug)
        .with_colors(true)
        .with_utc_timestamps()
        .init()
        .unwrap();
}

fn main() {
    init_logger();
    let input_device = select_audio_device();

    let stop_recording = Arc::new(AtomicBool::new(false));
    {
        let stop_recording = stop_recording.clone();
        ctrlc::set_handler(move || {
            stop_recording.store(true, Ordering::SeqCst);
        })
        .unwrap();
    }

    let _handle = recording::start_detector_thread(
        |info| {
            println!("beat: {info:?}");
        },
        Some(input_device),
    )
    .unwrap();

    log::info!("Start recording");
    while !stop_recording.load(Ordering::SeqCst) {}
    log::info!("Stopped recording");
}

/// Returns all valid and available input devices.
fn get_input_devices() -> Vec<(cpal::HostId, Vec<cpal::Device>)> {
    cpal::available_hosts()
        .into_iter()
        .map(|host_id| {
            let host = cpal::host_from_id(host_id).expect("should know the just queried host");
            (host_id, host)
        })
        .map(|(host_id, host)| (host_id, host.devices()))
        .filter(|(_, devices)| devices.is_ok())
        .map(|(host_id, devices)| (host_id, devices.unwrap()))
        .map(|(host_id, devices)| {
            (
                host_id,
                devices
                    .into_iter()
                    // check: is input device?
                    .filter(|dev| dev.default_input_config().is_ok())
                    // check: can we get its name?
                    .filter(|dev| dev.name().is_ok())
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>()
}

fn get_input_devices_flat() -> Vec<(cpal::HostId, cpal::Device)> {
    get_input_devices()
        .into_iter()
        .flat_map(|(host_id, devices)| {
            devices
                .into_iter()
                .map(|dev| (host_id, dev))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>()
}

/// Prompts the user in the terminal to choose an audio backend.
fn select_audio_device() -> cpal::Device {
    let mut devices = get_input_devices_flat();

    if devices.is_empty() {
        println!("No audio input device available");
        exit(0);
    }

    if devices.len() == 1 {
        return devices.swap_remove(0).1;
    }

    println!("Available input devices:");
    for (device_i, (host_id, device)) in devices.iter().enumerate() {
        println!(
            "[{}]: {:?} - {}",
            device_i,
            host_id,
            device
                .name()
                .expect("should be existent at that point due to the filtering")
        );
    }

    print!("Type a number: ");
    let mut buf = [0];
    std::io::stdin().read_exact(&mut buf).unwrap();
    println!(); // newline
    let buf = std::str::from_utf8(&buf).unwrap();
    let choice = usize::from_str_radix(buf, 10).unwrap();

    // Remove element and take ownership.
    devices.swap_remove(choice).1
}
