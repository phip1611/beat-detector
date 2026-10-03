// SPDX-License-Identifier: MIT

//! Module for audio recording from an audio input device.

use crate::{BeatDetector, BeatInfo};
use core::fmt::{Display, Formatter};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, StreamConfig};
use std::error::Error;
use std::string::ToString;
use std::time::{Duration, Instant};

/// Errors of [`start_detector_thread`].
#[derive(Debug)]
pub enum StartDetectorThreadError {
    /// There was no audio device provided and no default device can be found.
    NoDefaultAudioDevice,
    /// The audio backend failed to set up or start the input stream.
    Cpal(cpal::Error),
}

impl Display for StartDetectorThreadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoDefaultAudioDevice => f.write_str("no default audio input device"),
            Self::Cpal(_) => f.write_str("audio backend error"),
        }
    }
}

impl std::error::Error for StartDetectorThreadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::NoDefaultAudioDevice => None,
            Self::Cpal(err) => Some(err),
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
        input_dev
            .description()
            .map(|d| d.name().to_string())
            .unwrap_or_else(|_| "<unknown>".to_string())
    );

    let supported_input_config = input_dev
        .default_input_config()
        .map_err(StartDetectorThreadError::Cpal)?;

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

    let sampling_rate = input_config.sample_rate as f32;
    let mut detector = BeatDetector::new(sampling_rate, true);

    // Under the hood, this spawns a thread.
    let stream = input_dev
        .build_input_stream(
            input_config,
            move |data: &[i16], _info| {
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
        .map_err(StartDetectorThreadError::Cpal)?;

    stream.play().map_err(StartDetectorThreadError::Cpal)?;

    Ok(stream)
}
