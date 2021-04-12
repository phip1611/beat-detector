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
//! Module for audio recording from an audio input device.
//! This needs `std`-functionality.

use crate::{BeatInfo, StrategyKind};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, Device, Host, InputCallbackInfo, SampleFormat, StreamConfig, StreamError};
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{spawn, JoinHandle};
use std::time::Instant;

/// Starts listening to audio events and tries to recognize beats
/// on the audio. On each recognized beat, the specified callback
/// is executed. It does so by starting a new thread.
pub fn start_listening(
    on_beat_cb: impl Fn(BeatInfo) + Send + 'static,
    input_dev: Option<Device>,
    strategy: StrategyKind,
    keep_recording: Arc<AtomicBool>,
) -> Result<JoinHandle<()>, String> {
    if !keep_recording.load(Ordering::SeqCst) {
        return Err("Variable keep_recording is false from the beginning!?".to_string());
    }

    // we either use the "cpal" audio device the user wants to use
    // or otherwise the default input device
    let in_dev = input_dev.map(|d| Ok(d)).unwrap_or_else(|| {
        let host = cpal::default_host();
        host.default_input_device()
            .ok_or("Must have input device!".to_string())
    })?;
    let in_dev_cfg = in_dev.default_input_config().unwrap();
    // let channels = in_dev_cfg.channels();
    let sampling_rate = in_dev_cfg.sample_rate();
    let sample_format = in_dev_cfg.sample_format();
    eprintln!("Using input device: {:?}", in_dev.name().unwrap());
    // eprintln!("  channels: {}", channels);
    eprintln!("  sampling_rate: {}", sampling_rate.0);
    eprintln!("  sample_format: {:?}", sample_format);

    let err_cb = |err: StreamError| {
        eprintln!("Record error occurred: {:#?}", err);
    };

    // 1/44100 * 1024 = 23.22ms
    let preferred_window_length = 1024;

    let in_stream_cfg = StreamConfig {
        channels: 1,
        sample_rate: sampling_rate,
        #[cfg(not(target_os = "linux"))]
        buffer_size: BufferSize::Fixed(preferred_window_length),
        // on Raspberry Pi I can't set a fixed size, there are
        // "Illegal Argument" errors from ALSA; it works
        // on Mac and Windows tho
        #[cfg(target_os = "linux")]
        buffer_size: BufferSize::Default,
    };

    let detector = strategy.detector(sampling_rate.0);

    let handle = spawn(move || {
        // abstraction over possible return types
        // map all to [i16] and then do the appropriate callback
        let stream = match sample_format {
            SampleFormat::F32 => in_dev.build_input_stream(
                &in_stream_cfg,
                move |data: &[f32], _info: &InputCallbackInfo| {
                    let now = Instant::now();
                    if let Some(info) = detector.is_beat(&f32_data_to_i16(data)) {
                        on_beat_cb(info);
                    }
                    let millis = now.elapsed().as_millis();
                    if millis > 20 {
                        eprintln!("calculation took {}ms", millis);
                    }
                },
                err_cb,
            ),
            SampleFormat::I16 => in_dev.build_input_stream(
                &in_stream_cfg,
                move |data: &[i16], _info: &InputCallbackInfo| {
                    let now = Instant::now();
                    if let Some(info) = detector.is_beat(data) {
                        on_beat_cb(info);
                    }
                    let millis = now.elapsed().as_millis();
                    if millis > 20 {
                        eprintln!("calculation took {}ms", millis);
                    }
                },
                err_cb,
            ),
            SampleFormat::U16 => in_dev.build_input_stream(
                &in_stream_cfg,
                move |data: &[u16], _info: &InputCallbackInfo| {
                    let now = Instant::now();
                    if let Some(info) = detector.is_beat(&u16_data_to_i16(data)) {
                        on_beat_cb(info);
                    }
                    let millis = now.elapsed().as_millis();
                    if millis > 20 {
                        eprintln!("calculation took {}ms", millis);
                    }
                },
                err_cb,
            ),
        }
        .map_err(|err| format!("Can't open stream: {:?}", err))
        .unwrap();

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
        .map(|x| x - i16::MAX as i32 / 2)
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
pub fn audio_input_device_list() -> BTreeMap<String, Device> {
    let host = cpal::default_host();
    let devs = host.input_devices().unwrap().collect::<Vec<_>>();
    let mut map = BTreeMap::new();
    for (i, dev) in devs.into_iter().enumerate() {
        map.insert(dev.name().unwrap_or(format!("Unknown device #{}", i)), dev);
    }
    map
}

/// Convenient function which helps you to get capabilities of
/// each audio device covered by "cpal" audio library.
pub fn print_audio_input_device_configs() {
    let host = cpal::default_host();
    let devs = host.input_devices().unwrap().collect::<Vec<_>>();
    for (i, dev) in devs.into_iter().enumerate() {
        eprintln!("--------");
        let name = dev.name().unwrap_or(format!("Unknown device #{}", i));
        eprintln!("[{}] default config:", name);
        eprintln!("{:#?}", dev.default_input_config().unwrap());
        // eprintln!("[{}] available input configs:", name);
        // eprintln!("{:#?}", dev.supported_input_configs().unwrap());
    }
}

pub fn get_backends() -> HashMap<String, Host> {
    cpal::available_hosts()
        .into_iter()
        .map(|id| (format!("{:?}", id), cpal::host_from_id(id).unwrap()))
        .collect::<HashMap<_, _>>()
}

#[cfg(test)]
mod tests {
    use super::*;
}
