//! Module for audio recording from an audio input device.

use std::thread::{JoinHandle, spawn};
use cpal::traits::{HostTrait, DeviceTrait, StreamTrait};
use cpal::{Device, InputCallbackInfo, SampleFormat, StreamConfig, BufferSize, StreamError};

pub fn start_listening(
    on_beat_cb: impl Fn(&[i16], &InputCallbackInfo) + Send + 'static,
    input_dev: Option<Device>,
) -> Result<JoinHandle<()>, &'static str> {
    // we either use the "cpal" audio device the user wants to use
    // or otherwise the default input device
    let in_dev = input_dev.map(|d| Ok(d)).unwrap_or_else(|| {
        let host = cpal::default_host();
        // host.default_input_device().ok_or("Must have input device!")
        for (i, d) in host.input_devices().unwrap().enumerate() {
            if i == 1 {
                return Ok(d)
            }
        }
        Err("")
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
        eprintln!("Record error occured: {:#?}", err);
    };

    let in_stream_cfg = StreamConfig {
        channels: 1,
        sample_rate: sampling_rate,
        buffer_size: BufferSize::Fixed(1024),
    };

    let handle = spawn(move || {
        // abstraction over possible return types
        // map all to [i16] and then do the appropriate callback
        let stream = match sample_format {
            SampleFormat::F32 => in_dev.build_input_stream(
                &in_stream_cfg,
                move |data: &[f32], info: &InputCallbackInfo| {
                    let samples = f32_data_to_i16(data);
                    on_beat_cb(&samples, info);
                },
                err_cb,
            ).map_err(|err| "Can't open stream").unwrap(),
            SampleFormat::I16 => in_dev.build_input_stream(
                &in_stream_cfg,
                move |data: &[i16], info: &InputCallbackInfo| {
                    on_beat_cb(&data, info);
                },
                err_cb,
            ).map_err(|err| "Can't open stream").unwrap(),
            SampleFormat::U16 => in_dev.build_input_stream(
                &in_stream_cfg,
                move |data: &[u16], info: &InputCallbackInfo| {
                    let samples = u16_data_to_i16(data);
                    on_beat_cb(&samples, info);
                },
                err_cb,
            ).map_err(|err| "Can't open stream").unwrap(),
        };

        stream.play().unwrap();

        // TODO add CTRL+C interruption
        loop {}
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_start_listening() {
        let h = start_listening(|data: &[i16], info: &InputCallbackInfo| {
            for x in data {
                if *x > 1000 {
                    // make some noise; you should see something
                    println!("{}", x);
                }
            }
        }, None).unwrap();
        let h = h.join().unwrap();
    }

    #[test]
    fn test_start_listening_recognize_beats() {
        let lpf_det: _ = crate::StrategyKind::LPF.detector(44100, 1024);
        let spectrum_det: _ = crate::StrategyKind::Spectrum.detector(44100, 1024);
        let h = start_listening(move |data: &[i16], info: &InputCallbackInfo| {
            assert_eq!(data.len(), 1024);
            let b1 = lpf_det.is_beat(&data);
            let b2 = spectrum_det.is_beat(&data);
            if b1.is_some() {
                println!("Lowpass filter strategy recognized a beat!");
            }
            if b2.is_some() {
                println!("spectrum strategy recognized a beat!");
            }
        }, None).unwrap();
        let h = h.join().unwrap();
    }
}