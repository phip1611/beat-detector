use beat_detector::{DownsamplingMetrics, ValidInputFrequencies};
use std::time::Duration;

fn main() {
    println!("Helper to identify the effective audio buffer length for a common");
    println!("set of sample rates, cutoff frequencies, and the downsampling ");
    println!("strategy used by this crate for reduced memory and computing");
    println!("load.");

    let common_sample_rates = [
        8000, 11025, 16000, 22050, 32000, 44100, 48000, 88200, 96000, 176400, 192000,
    ];
    let common_cutoff_frequencies = [60, 80, 100, 120];
    let duration = Duration::from_millis(1000);

    println!();
    println!("To store 500ms of audio history (after lowpass and downsampling you need:");
    println!();

    for sample_rate in common_sample_rates {
        for cutoff_frequency in common_cutoff_frequencies {
            let input =
                ValidInputFrequencies::new(sample_rate as f32, cutoff_frequency as f32).unwrap();
            let metrics = DownsamplingMetrics::new(input);
            let effective_sample_rate = metrics.effective_sample_rate_hz();

            let sample_count = effective_sample_rate.raw() * duration.as_secs_f32();
            let size = sample_count as usize * size_of::<i16>();

            println!("sample rate (input): {sample_rate}");
            println!("cutoff_frequency   : {cutoff_frequency}");
            println!("sample rate (eff)  : {effective_sample_rate}");
            println!("memory (bytes)     : {size} (i16 mono samples)");
            println!();
        }
    }
}
