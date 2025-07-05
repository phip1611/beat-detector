//! Module for convenient handling of primitive [`f32`] types.
//!
//! In a nutshell, this exports restricted type wrappers around [`f32`] values
//! with certain guarantees to be valid numbers.

use core::fmt::{Display, Formatter};
use core::ops::RangeInclusive;
use thiserror::Error;

/// The underlying value is not valid, i.e., not finite or not in a finite
/// range.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum InvalidF32Error {
    #[error("value is not a number (NaN)")]
    NAN,
    #[error("value is infinite")]
    Infinite,
    #[error("finite value {0} is not in finite range {1:?}")]
    NotInRange(f32 /* finite */, RangeInclusive<f32>),
    #[error("has fractional part when this was not expected: {0}")]
    HasFractionalPart(f32),
}

/// A finite f32 that is [`Ord`] and [`Eq`].
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
struct FiniteF32(f32 /* finite: not NaN or infinite */);

impl Ord for FiniteF32 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.partial_cmp(&other.0).unwrap()
    }
}

impl Eq for FiniteF32 {}

impl TryFrom<f32> for FiniteF32 {
    type Error = InvalidF32Error;

    fn try_from(value: f32) -> Result<Self, Self::Error> {
        if value.is_nan() {
            Err(InvalidF32Error::NAN)
        } else if value.is_infinite() {
            Err(InvalidF32Error::Infinite)
        } else {
            assert!(value.is_finite());
            Ok(Self(value))
        }
    }
}

fn f32_is_finite_and_in_range(
    value: f32,
    range: RangeInclusive<f32>,
) -> Result<FiniteF32, InvalidF32Error> {
    let finite_f32 = FiniteF32::try_from(value)?;
    if range.contains(&finite_f32.0) {
        Ok(finite_f32)
    } else {
        Err(InvalidF32Error::NotInRange(value, range))
    }
}

/// A frequency in range `0.0..Infinity` (Hertz).
///
/// Note that for simplicity only even frequencies are supported, i.e., no
/// numbers with a fractional part.
///
/// Typically, this is a value such as `44100.0`.
#[derive(Copy, Clone, Debug, PartialOrd, PartialEq, Ord, Eq)]
#[repr(transparent)]
pub struct F32Frequency(FiniteF32);

impl F32Frequency {
    const VALID_RANGE: RangeInclusive<f32> = 0.0..=f32::MAX;

    /// Returns the underlying raw value.
    pub const fn raw(self) -> f32 {
        self.0 .0
    }
}

impl TryFrom<f32> for F32Frequency {
    type Error = InvalidF32Error;

    fn try_from(value: f32) -> Result<Self, Self::Error> {
        // base assertions
        let _ = FiniteF32::try_from(value)?;

        // check negative zero
        if value.is_sign_negative() {
            Err(InvalidF32Error::NotInRange(value, Self::VALID_RANGE))
        } else if value.fract() != 0.0 {
            Err(InvalidF32Error::HasFractionalPart(value))
        } else {
            f32_is_finite_and_in_range(value, Self::VALID_RANGE).map(Self)
        }
    }
}

impl Display for F32Frequency {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} Hz", self.raw())
    }
}

/// A sample value in range `-1.0..=1.0` (amplitude).
#[derive(Copy, Clone, Debug, PartialOrd, PartialEq, Ord, Eq)]
#[repr(transparent)]
pub struct F32Sample(FiniteF32);

impl F32Sample {
    const VALID_RANGE: RangeInclusive<f32> = -1.0..=1.0;

    /// Returns the underlying value.
    pub const fn raw(self) -> f32 {
        self.0 .0
    }
}

impl TryFrom<f32> for F32Sample {
    type Error = InvalidF32Error;

    fn try_from(value: f32) -> Result<Self, Self::Error> {
        f32_is_finite_and_in_range(value, Self::VALID_RANGE).map(Self)
    }
}

impl Display for F32Sample {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.raw())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_f32_finite() {
        assert_eq!(FiniteF32::try_from(0.0), Ok(FiniteF32(0.0)));
        assert_eq!(FiniteF32::try_from(-0.0), Ok(FiniteF32(-0.0)));
        assert_eq!(FiniteF32::try_from(-42.0), Ok(FiniteF32(-42.0)));
        assert_eq!(FiniteF32::try_from(42.0), Ok(FiniteF32(42.0)));

        assert_eq!(FiniteF32::try_from(f32::NAN), Err(InvalidF32Error::NAN));
        assert_eq!(FiniteF32::try_from(-f32::NAN), Err(InvalidF32Error::NAN));
        assert_eq!(
            FiniteF32::try_from(f32::INFINITY),
            Err(InvalidF32Error::Infinite)
        );
        assert_eq!(
            FiniteF32::try_from(-f32::NEG_INFINITY),
            Err(InvalidF32Error::Infinite)
        );
    }

    #[test]
    fn test_type_f32_frequency() {
        // Base sanity checks
        {
            assert_eq!(F32Frequency::try_from(f32::NAN), Err(InvalidF32Error::NAN));
            assert_eq!(F32Frequency::try_from(-f32::NAN), Err(InvalidF32Error::NAN));
            assert_eq!(
                F32Frequency::try_from(f32::INFINITY),
                Err(InvalidF32Error::Infinite)
            );
            assert_eq!(
                F32Frequency::try_from(-f32::NEG_INFINITY),
                Err(InvalidF32Error::Infinite)
            );
        }

        assert_eq!(
            F32Frequency::try_from(-0.0),
            Err(InvalidF32Error::NotInRange(-0.0, F32Frequency::VALID_RANGE))
        );
        assert_eq!(
            F32Frequency::try_from(-42.0),
            Err(InvalidF32Error::NotInRange(
                -42.0,
                F32Frequency::VALID_RANGE
            ))
        );

        assert_eq!(
            F32Frequency::try_from(0.0),
            Ok(F32Frequency(FiniteF32(0.0)))
        );
        assert_eq!(
            F32Frequency::try_from(42.0),
            Ok(F32Frequency(FiniteF32(42.0)))
        );

        assert_eq!(
            F32Frequency::try_from(42.4),
            Err(InvalidF32Error::HasFractionalPart(42.4))
        );
    }

    #[test]
    fn test_type_f32_sample() {
        assert_eq!(F32Sample::try_from(f32::NAN), Err(InvalidF32Error::NAN));
        assert_eq!(F32Sample::try_from(-f32::NAN), Err(InvalidF32Error::NAN));
        assert_eq!(
            F32Sample::try_from(f32::INFINITY),
            Err(InvalidF32Error::Infinite)
        );
        assert_eq!(
            F32Sample::try_from(-f32::NEG_INFINITY),
            Err(InvalidF32Error::Infinite)
        );

        assert_eq!(
            F32Sample::try_from(-1.1),
            Err(InvalidF32Error::NotInRange(-1.1, F32Sample::VALID_RANGE))
        );
        assert_eq!(
            F32Sample::try_from(1.1),
            Err(InvalidF32Error::NotInRange(1.1, F32Sample::VALID_RANGE))
        );

        assert_eq!(F32Sample::try_from(0.0), Ok(F32Sample(FiniteF32(0.0))));
        assert_eq!(F32Sample::try_from(-0.0), Ok(F32Sample(FiniteF32(-0.0))));
        assert_eq!(F32Sample::try_from(-1.0), Ok(F32Sample(FiniteF32(-1.0))));
        assert_eq!(F32Sample::try_from(1.0), Ok(F32Sample(FiniteF32(1.0))));
    }
}
