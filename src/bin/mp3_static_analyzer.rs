use minimp3::{Decoder, Error, Frame};

use std::fs::File;
use std::io::Write;
use synthrs::filter::{convolve, cutoff_from_frequency, lowpass_filter};
use synthrs::synthesizer::quantize_samples;

fn main() {
    let mut decoder = Decoder::new(File::open("res/sample_1.mp3").unwrap());

    let mut timestamped_lowpassed_data = vec![];

    let sample_duration_in_s = 1_f64 / 44100_f64;
    let mut i = 0;
    loop {
        match decoder.next_frame() {
            // a frame has 1152 samples [left @ index 0,right @ index 1,left,right,left...right @ index 1151]
            // this means we have "1/44100Hz * (1152 samples/frame / 2)" [576 x l, 576 x r, in lrlrlr... order]
            // => 0.013s => 13ms
            // for the beginning I assume that a beat takes ~60ms
            Ok(Frame {
                data,
                sample_rate,
                channels,
                ..
            }) => {
                // println!("Decoded {} samples @ {}", data.len() / channels, sample_rate)
                let mono_data = stereo_to_mono(data);
                let lowpassed_data = mono_through_lowpass(mono_data);
                let abs_data = lowpassed_data
                    .into_iter()
                    .map(|x| if x < 0 { -x as u16 } else { x as u16 })
                    .collect::<Vec<u16>>();

                let frame_time_seconds = (i as f64) * sample_duration_in_s * 1152_f64;
                for (sample_i, sample) in abs_data.iter().enumerate() {
                    let sample_time_seconds = sample_duration_in_s * sample_i as f64;
                    let total_time_seconds = frame_time_seconds + sample_time_seconds;
                    timestamped_lowpassed_data.push((total_time_seconds, *sample));
                }

                i += 1;
            }
            Err(Error::Eof) => break,
            Err(e) => panic!("{:?}", e),
        }
    }

    let mut csv = String::new();
    csv += "timestamp;sample_val;\n";
    for (timestamp, val) in timestamped_lowpassed_data.iter().step_by(50) {
        csv += &format!("{}s;{};\n", timestamp, val);
    }

    let mut f = File::create("analysis.csv").expect("Unable to create file");
    f.write_all(csv.as_bytes()).expect("Unable to write data");
}

/// Takes the LRLRLR data from a frame and transforms all samples to mono sound.
fn stereo_to_mono(stereo_lrlr_data: Vec<i16>) -> Vec<i16> {
    assert_eq!(stereo_lrlr_data.len() % 2, 0, "must be a multiple of 2");
    let mut mono_data = vec![];
    for i in 0..stereo_lrlr_data.len() / 2 {
        let sum = (stereo_lrlr_data[i] as i32) + (stereo_lrlr_data[i + 1] as i32);
        let avg = (sum / 2) as i16;
        mono_data.push(avg);
    }
    assert!(
        mono_data.len() == (stereo_lrlr_data.len() / 2),
        "must transform stereo to mono for all samples of the frame"
    );
    mono_data
}

fn mono_through_lowpass(mono_data_samples: Vec<i16>) -> Vec<i16> {
    // Create a lowpass filter, using a cutoff of 400Hz at a 44_100Hz sample rate (ie. filter out frequencies >400Hz)
    let lowpass = lowpass_filter(cutoff_from_frequency(100.0, 44_100), 0.01);

    // Apply convolution to filter out high frequencies
    let lowpass_samples = quantize_samples::<i16>(&convolve(
        &lowpass,
        &mono_data_samples
            .iter()
            .map(|a| *a as f64)
            .collect::<Vec<f64>>(),
    ));

    lowpass_samples
}
