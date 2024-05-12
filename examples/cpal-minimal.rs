use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, InputCallbackInfo, StreamConfig};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[path = "_modules/example_utils.rs"]
mod example_utils;

/// Minimal example to explore the structure of the audio input samples we get
/// from cpal. This example does nothing with the beat detection library.
fn main() {
    let host = cpal::default_host();
    let dev = host.default_input_device().unwrap();
    let x = dev
        .supported_input_configs()
        .unwrap()
        .into_iter()
        .map(|r| r)
        .collect::<Vec<_>>();
    dbg!(x);
    let cfg = dev.default_input_config().unwrap();
    let cfg = StreamConfig {
        channels: 1,
        sample_rate: cfg.sample_rate(),
        buffer_size: BufferSize::Default,
    };

    let mut max = i16::MIN;
    let mut min = i16::MAX;

    let stream = dev
        .build_input_stream(
            &cfg,
            // cpal is powerful enough to let us specify the type of the
            // samples, such as `&[i16]` or `&[f32]`. For i16, the value is
            // between `i16::MIN..i16::MAX`, for f32, the value is between
            // `-1.0..1.0`. Supported formats are in enum `SampleFormat`.
            move |samples: &[i16], _info| {
                for &sample in samples {
                    max = core::cmp::max(max, sample);
                    min = core::cmp::min(min, sample);
                    println!("{sample:>6}, max={max:>6}, min={min:>6}");
                }
            },
            |info| {},
            None,
        )
        .unwrap();

    let stop_recording = Arc::new(AtomicBool::new(false));
    {
        let stop_recording = stop_recording.clone();
        ctrlc::set_handler(move || {
            stop_recording.store(true, Ordering::SeqCst);
        })
        .unwrap();
    }

    stream.play().unwrap();
    while !stop_recording.load(Ordering::SeqCst) {}
    stream.pause().unwrap();
}
