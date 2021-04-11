use cpal::Device;
use std::collections::HashMap;
use std::io::stdin;
use beat_detector::StrategyKind;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn main() {
    let recording = Arc::new(AtomicBool::new(true));
    let recording_cpy = recording.clone();
    ctrlc::set_handler(move || {
        eprintln!("Stopping recording");
        recording_cpy.store(false, Ordering::SeqCst);
    }).expect("Ctrl-C handler doesn't work");

    let devs = beat_detector::record::audio_input_device_list();
    if devs.is_empty() { panic!("No audio input devices found!") }
    let dev = if devs.len() > 1 { select_input_device(devs) } else { devs.into_iter().next().unwrap().1 };
    let strategy = select_strategy();
    let on_beat = |info| {
        println!("Found beat at {:?}ms", info);
    };
    let handle = beat_detector::record::start_listening(
        on_beat,
        Some(dev),
        strategy,
        recording,
    ).unwrap();

    handle.join().unwrap();
}

fn select_input_device(devs: HashMap<String, Device>) -> Device {
    println!("Available audio devices:");
    for (i, (name, _)) in devs.iter().enumerate() {
        println!("  [{}]{}", i, name);
    }
    println!("Select audio device: input device number and enter:");
    let mut input = String::new();
    while stdin().read_line(&mut input).unwrap() == 0 {}
    let input = input.trim().parse::<u8>().expect("Input must be a valid number!");
    devs.into_iter().enumerate()
        .filter(|(i, _)| *i == input as usize)
        .map(|(_i, (_name, dev))| dev)
        .take(1)
        .next()
        .unwrap()
}

fn select_strategy() -> StrategyKind {
    println!("Available beat detection strategies:");
    println!("  [0] Lowpass Filter");
    println!("  [1] Spectrum Analysis");
    println!("Select strategy: input id and enter:");
    let mut input = String::new();
    while stdin().read_line(&mut input).unwrap() == 0 {}
    let input = input.trim().parse::<u8>().expect("Input must be a valid number!");
    match input {
        0 => StrategyKind::LPF,
        1 => StrategyKind::Spectrum,
        _ => panic!("Invalid strategy!"),
    }
}