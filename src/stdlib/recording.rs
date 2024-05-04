/*
MIT License

Copyright (c) 2024 Philipp Schuster

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

use super::*;
use crate::{BeatDetector, BeatInfo};
use core::fmt::{Display, Formatter};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, StreamConfig};
use std::error::Error;
use std::time::{Duration, Instant};

#[derive(Debug)]
// #[derive(Debug, Clone)]
pub enum StartDetectorThreadError {
    /// There was no audio device provided and no default device can be found.
    NoDefaultAudioDevice,
    /// There was a problem detecting the input stream config.
    InputConfigError(cpal::DefaultStreamConfigError),
    /// Failed to build an input stream.
    FailedBuildingInputStream(cpal::BuildStreamError),
    /// There was a problem
    InputError(cpal::PlayStreamError),
}

impl Display for StartDetectorThreadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{:?}", self))
    }
}

impl std::error::Error for StartDetectorThreadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InputConfigError(err) => Some(err),
            Self::FailedBuildingInputStream(err) => Some(err),
            Self::InputError(err) => Some(err),
            _ => None,
        }
    }
}

/// Starts a stream (a thread) that combines the audio input with the provided
/// callback. The stream lives as long as the provided callback
pub fn start_detector_thread(
    on_beat_cb: impl Fn(BeatInfo) + Send + 'static,
    preferred_input_dev: Option<cpal::Device>,
) -> Result<cpal::Stream, StartDetectorThreadError> {
    let input_dev = preferred_input_dev.map(Ok).unwrap_or_else(|| {
        let host = cpal::default_host();
        log::debug!("Using '{:?}' as input framework", host.id());
        host.default_input_device()
            .ok_or(StartDetectorThreadError::NoDefaultAudioDevice)
    })?;

    log::debug!(
        "Using '{}' as input device",
        input_dev.name().unwrap_or_else(|_| "<unknown>".to_string())
    );

    let supported_input_config = input_dev
        .default_input_config()
        .map_err(StartDetectorThreadError::InputConfigError)?;

    log::trace!(
        "Supported input configurations: {:#?}",
        supported_input_config
    );

    let input_config = StreamConfig {
        channels: 1,
        sample_rate: supported_input_config.sample_rate(),
        //buffer_size: get_desired_frame_count_if_possible(),
        buffer_size: BufferSize::Default,
    };

    log::debug!("Input configuration: {:#?}", input_config);

    let sampling_rate = input_config.sample_rate.0 as f32;
    let mut detector = BeatDetector::new(sampling_rate, true);

    // Under the hood, this spawns a thread.
    let stream = input_dev
        .build_input_stream(
            &input_config,
            move |data: &[f32], _info| {
                log::trace!(
                    "audio input callback: {} samples ({} ms, sampling rate = {sampling_rate})",
                    data.len(),
                    Duration::from_secs_f32(data.len() as f32 / sampling_rate).as_millis()
                );

                let now = Instant::now();
                let beat = detector.update_and_detect_beat(data.iter().copied());
                let duration = now.elapsed();
                log::trace!("Beat detection took {:?}", duration);

                if let Some(beat) = beat {
                    log::debug!("Beat detection took {:?}", duration);
                    on_beat_cb(beat);
                }
            },
            |e| {
                log::error!("Input error: {e:#?}");
            },
            // Timeout: worst case max blocking time
            // Don't see too short, as otherwise, the error callback will be
            // invoked frequently.
            // https://github.com/RustAudio/cpal/pull/696
            Some(Duration::from_secs(1)),
        )
        .map_err(StartDetectorThreadError::FailedBuildingInputStream)?;

    stream
        .play()
        .map_err(StartDetectorThreadError::InputError)?;

    Ok(stream)
}
