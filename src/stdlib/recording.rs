// SPDX-License-Identifier: MIT

//! Beat detection on the audio input of the system, using [`cpal`].

use crate::detector::{Beat, BeatDetector};
use core::fmt::{Display, Formatter};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};
use std::error::Error;
use std::string::ToString;
use std::time::{Duration, Instant};
use std::vec::Vec;

/// Errors of [`start_detector_thread`].
#[derive(Debug)]
pub enum StartDetectorThreadError {
    /// There was no audio device provided and no default device can be found.
    NoDefaultAudioDevice,
    /// The audio device delivers samples in a format that isn't supported.
    UnsupportedSampleFormat(SampleFormat),
    /// The audio backend failed to set up or start the input stream.
    Cpal(cpal::Error),
}

impl Display for StartDetectorThreadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoDefaultAudioDevice => f.write_str("no default audio input device"),
            Self::UnsupportedSampleFormat(format) => {
                write!(f, "unsupported sample format: {format}")
            }
            Self::Cpal(_) => f.write_str("audio backend error"),
        }
    }
}

impl std::error::Error for StartDetectorThreadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::NoDefaultAudioDevice | Self::UnsupportedSampleFormat(_) => None,
            Self::Cpal(err) => Some(err),
        }
    }
}

/// Starts beat detection on an audio input device and calls `on_beat` for
/// each beat.
///
/// Uses the default input device of the system if `preferred_input_dev` is
/// `None`. Multi-channel input is mixed down to mono. Detection runs as long
/// as the returned stream lives.
pub fn start_detector_thread(
    on_beat: impl FnMut(Beat) + Send + 'static,
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

    let supported_config = input_dev
        .default_input_config()
        .map_err(StartDetectorThreadError::Cpal)?;
    log::debug!("Input configuration: {supported_config:#?}");

    let format = supported_config.sample_format();
    let config = supported_config.config();
    let stream = match format {
        SampleFormat::F32 => build_stream::<f32>(&input_dev, config, on_beat),
        SampleFormat::I16 => build_stream::<i16>(&input_dev, config, on_beat),
        SampleFormat::I32 => build_stream::<i32>(&input_dev, config, on_beat),
        SampleFormat::U16 => build_stream::<u16>(&input_dev, config, on_beat),
        format => return Err(StartDetectorThreadError::UnsupportedSampleFormat(format)),
    }?;
    stream.play().map_err(StartDetectorThreadError::Cpal)?;
    Ok(stream)
}

fn build_stream<T>(
    input_dev: &cpal::Device,
    config: StreamConfig,
    mut on_beat: impl FnMut(Beat) + Send + 'static,
) -> Result<cpal::Stream, StartDetectorThreadError>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = usize::from(config.channels);
    let sample_rate = config.sample_rate as f32;
    let mut detector = BeatDetector::new(sample_rate);
    // Reused across callbacks to not allocate on the audio thread.
    let mut mono = Vec::new();

    input_dev
        .build_input_stream(
            config,
            move |data: &[T], _info| {
                let now = Instant::now();

                mono.clear();
                mono.extend(data.chunks_exact(channels).map(|frame| {
                    frame.iter().map(|s| s.to_sample::<f32>()).sum::<f32>() / channels as f32
                }));
                for chunk in mono.chunks(detector.max_chunk_len()) {
                    if let Some(beat) = detector.process(chunk) {
                        on_beat(beat);
                    }
                }

                // Visible with RUST_LOG=trace in the examples.
                log::trace!(
                    "Processed {:.1} ms of audio in {:?}",
                    mono.len() as f32 / sample_rate * 1000.0,
                    now.elapsed()
                );
            },
            |e| {
                log::error!("Input error: {e:#?}");
            },
            // Timeout: worst case max blocking time
            // Don't set too short, as otherwise, the error callback will be
            // invoked frequently.
            // https://github.com/RustAudio/cpal/pull/696
            Some(Duration::from_secs(1)),
        )
        .map_err(StartDetectorThreadError::Cpal)
}
