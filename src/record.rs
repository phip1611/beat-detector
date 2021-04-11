//! Module for audio recording from an audio input device.
//! This needs `std`-functionality.

use std::thread::{JoinHandle, spawn};
use cpal::traits::{HostTrait, DeviceTrait, StreamTrait};
use cpal::{Device, InputCallbackInfo, SampleFormat, StreamConfig, BufferSize, StreamError};
use crate::{StrategyKind, BeatInfo};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Starts listening to audio events and tries to recognize beats
/// on the audio. On each recognized beat, the specified callback
/// is executed. It does so by starting a new thread.
pub fn start_listening(
    on_beat_cb: impl Fn(BeatInfo) + Send + 'static,
    input_dev: Option<Device>,
    strategy: StrategyKind,
    keep_recording: Arc<AtomicBool>,
) -> Result<JoinHandle<()>, &'static str> {
    if !keep_recording.load(Ordering::SeqCst) {
        return Err("Variable keep_recording is false from the beginning!?");
    }

    // we either use the "cpal" audio device the user wants to use
    // or otherwise the default input device
    let in_dev = input_dev.map(|d| Ok(d)).unwrap_or_else(|| {
        let host = cpal::default_host();
        host.default_input_device().ok_or("Must have input device!")
    })?;
    let in_dev_cfg = in_dev.default_input_config().unwrap();
    let channels = in_dev_cfg.channels();
    let sampling_rate = in_dev_cfg.sample_rate();
    let sample_format = in_dev_cfg.sample_format();
    eprintln!("Using input device: {:?}", in_dev.name());
    eprintln!("  channels: {}", channels);
    eprintln!("  sampling_rate: {}", sampling_rate.0);
    eprintln!("  sample_format: {:?}", sample_format);

    let err_cb = |err: StreamError| {
        eprintln!("Record error occurred: {:#?}", err);
    };

    let window_length = 1024;

    let in_stream_cfg = StreamConfig {
        channels: 1,
        sample_rate: sampling_rate,
        buffer_size: BufferSize::Fixed(window_length),
    };

    let detector = strategy.detector(
        sampling_rate.0,
        window_length as u16
    );

    let handle = spawn(move || {
        // abstraction over possible return types
        // map all to [i16] and then do the appropriate callback
        let stream = match sample_format {
            SampleFormat::F32 => in_dev.build_input_stream(
                &in_stream_cfg,
                move |data: &[f32], _info: &InputCallbackInfo| {
                    let samples = f32_data_to_i16(data);
                    if let Some(info) = detector.is_beat(&samples) {
                        on_beat_cb(info);
                    }
                },
                err_cb,
            ).map_err(|_err| "Can't open stream").unwrap(),
            SampleFormat::I16 => in_dev.build_input_stream(
                &in_stream_cfg,
                move |data: &[i16], _info: &InputCallbackInfo| {
                    if let Some(info) = detector.is_beat(&data) {
                        on_beat_cb(info);
                    }
                },
                err_cb,
            ).map_err(|_err| "Can't open stream").unwrap(),
            SampleFormat::U16 => in_dev.build_input_stream(
                &in_stream_cfg,
                move |data: &[u16], _info: &InputCallbackInfo| {
                    let samples = u16_data_to_i16(data);
                    if let Some(info) = detector.is_beat(&samples) {
                        on_beat_cb(info);
                    }
                },
                err_cb,
            ).map_err(|_err| "Can't open stream").unwrap(),
        };

        // start input stream
        stream.play().unwrap();

        // start listening loop until stopped
        loop {
            if !keep_recording.load(Ordering::SeqCst) {
                break;
            }
        }
    });

    Ok(handle)
}

#[inline(always)]
fn u16_data_to_i16(data: &[u16]) -> Vec<i16> {
    data.iter()
        .map(|x| *x as i32)
        .map(|x| x - i16::MAX as i32/2)
        .map(|x| x as i16)
        .collect()
}

#[inline(always)]
fn f32_data_to_i16(data: &[f32]) -> Vec<i16> {
        data.iter()
            .map(|x| x * i16::MAX as f32)
            .map(|x| x as i16)
            .collect()
}

/// Convenient function which helps you to select from a number of
/// audio devices using "cpal" audio library.
pub fn audio_input_device_list() -> HashMap<String, Device> {
    let host = cpal::default_host();
    let devs = host.input_devices().unwrap().collect::<Vec<_>>();
    let mut map = HashMap::new();
    for (i, dev) in devs.into_iter().enumerate() {
        map.insert(
            dev.name().unwrap_or(format!("Unknown device #{}", i)),
            dev
        );
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;


}