use cpal::traits::DeviceTrait;

#[path = "_modules/example_utils.rs"]
mod example_utils;

/// Minimal example to explore the structure of the audio input samples we get
/// from cpal. This example does nothing with the beat detection library.
fn main() {
    let input_device = example_utils::select_audio_device();
    let supported_configs = input_device
        .supported_input_configs()
        .unwrap()
        .into_iter()
        .collect::<Vec<_>>();
    println!("Supported input configs:");
    for cfg in supported_configs {
        println!(
            "channels: {:>2}, format: {format:>3}, min_rate: {:06?}, max_rate: {:06?}, buffer: {:?}",
            cfg.channels(),
            cfg.min_sample_rate(),
            cfg.max_sample_rate(),
            cfg.buffer_size(),
            format = format!("{:?}", cfg.sample_format(),)
        );
    }
}
