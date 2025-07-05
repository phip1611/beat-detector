//! Necessary types, helpers, and functions to process audio input and to
//! prepare it for the **analysis layer**.
//!
//! This module only operates on raw data and data streams, without interacting
//! with the outer world (I/O).

use crate::layer_input_processing::f32::F32Frequency;
use thiserror::Error;

pub mod conversion;
pub mod downsampling;
pub mod f32;
pub mod lowpass_filter;
mod beat_detector;

/// Possible errors when working with [`ValidInputFrequencies`].
#[derive(Debug, Clone, PartialEq, Error)]
pub enum InvalidFrequencyError {
    /// The cutoff frequency doesn't fulfill the Nyquist rule.
    #[error("invalid cutoff frequency: {0} * 2 <= sampling rate ({1}) is not ")]
    InvalidCutoffFrequency(F32Frequency, F32Frequency),
    #[error("the f32 value is not a valid frequency value")]
    InvalidF32(#[from] self::f32::InvalidF32Error),
}

/// Represents a validated pair of sample rate and cutoff frequency.
#[derive(Debug, Clone, Copy, PartialOrd, PartialEq, Ord, Eq)]
pub struct ValidInputFrequencies {
    sample_rate_hz: F32Frequency,
    cutoff_fr_hz: F32Frequency,
}

impl ValidInputFrequencies {
    /// Creates a new struct of validated input frequencies.
    ///
    /// `cutoff_fr_hz` is typically a value below `120 Hz` for beat detection.
    pub fn new(sample_rate_hz: f32, cutoff_fr_hz: f32) -> Result<Self, InvalidFrequencyError> {
        let sample_rate_hz = F32Frequency::try_from(sample_rate_hz)?;
        let cutoff_fr_hz = F32Frequency::try_from(cutoff_fr_hz)?;

        // Check Nyquist
        if cutoff_fr_hz.raw() * 2.0 > sample_rate_hz.raw() {
            return Err(InvalidFrequencyError::InvalidCutoffFrequency(
                sample_rate_hz,
                cutoff_fr_hz,
            ));
        }

        Ok(Self {
            sample_rate_hz,
            cutoff_fr_hz,
        })
    }

    /// Returns the sample rate (Hz).
    #[must_use]
    pub fn sample_rate_hz(&self) -> F32Frequency {
        self.sample_rate_hz
    }

    /// Returns the cutoff frequency (Hz).
    #[must_use]
    pub fn cutoff_fr_hz(&self) -> F32Frequency {
        self.cutoff_fr_hz
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
