//! TODO is it worth it to refactor from f32 to sample? I should do this once
//! an initial working version is there.

use core::cmp::Ordering;
// use core::convert::Infallible;
use core::ops::Deref;
use core::ops::DerefMut;
use core::ops::{Add, Div, Mul, Sub};

/// The sample is not in the specified range.
#[derive(Debug, Copy, Clone)]
pub struct SampleRangeError(f32);

/// Lightweight wrapper around a `f32` that behaves as an `f32` and tightly
/// integrates with it (easy conversions), but guaranteed that constructed
/// instances are in the valid range [`Sample::MIN`] and [`Sample::MAX`].
///
/// Consequentially, calculus operations (+,-,*,/) return a new `f32` instance
/// which is more convenient for average calculations and so on, but not
/// necessarily a valid [`Sample`] anymore.
///
/// You can easily construct this from `f32`, easily convert it back, and easily
/// compare it with `f32` values.
#[derive(Copy, Clone, Debug)]
#[repr(transparent)]
pub struct Sample(f32);

impl Sample {
    /// Inclusive maximum value of a sample.
    const MAX: f32 = 1.0;
    /// Inclusive minimum value of a sample.
    const MIN: f32 = -1.0;

    /// Constructs a new sample.
    pub fn new(value: f32) -> Result<Self, SampleRangeError> {
        if value >= Self::MIN && value <= Self::MAX {
            Ok(Self(value))
        } else {
            Err(SampleRangeError(value))
        }
    }
}

impl Add for Sample {
    type Output = f32;

    fn add(self, rhs: Self) -> Self::Output {
        self.0 + rhs.0
    }
}

impl Sub for Sample {
    type Output = f32;

    fn sub(self, rhs: Self) -> Self::Output {
        self.0 - rhs.0
    }
}

impl Mul for Sample {
    type Output = f32;

    fn mul(self, rhs: Self) -> Self::Output {
        self.0 * rhs.0
    }
}

impl Div for Sample {
    type Output = f32;

    fn div(self, rhs: Self) -> Self::Output {
        self.0 / rhs.0
    }
}

impl Default for Sample {
    fn default() -> Self {
        Self(0.0)
    }
}

impl PartialEq for Sample {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl PartialEq<f32> for Sample {
    fn eq(&self, other: &f32) -> bool {
        self.0 == *other
    }
}

impl PartialEq<Sample> for f32 {
    fn eq(&self, other: &Sample) -> bool {
        self == &other.0
    }
}

impl PartialOrd for Sample {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl PartialOrd<f32> for Sample {
    fn partial_cmp(&self, other: &f32) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}

impl PartialOrd<Sample> for f32 {
    fn partial_cmp(&self, other: &Sample) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}

impl Eq for Sample {}

impl Ord for Sample {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other)
            .expect("should only have comparable value at this point (not NaN or so)")
    }
}

impl Deref for Sample {
    type Target = f32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Sample {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl AsRef<f32> for Sample {
    fn as_ref(&self) -> &f32 {
        &self.0
    }
}

impl From<f32> for Sample {
    fn from(value: f32) -> Self {
        Sample::new(value).unwrap()
    }
}

impl From<Sample> for f32 {
    fn from(value: Sample) -> Self {
        value.0
    }
}
/* conflicting implementation from core :/
impl TryFrom<f32> for Sample {
    type Error = SampleRangeError;

    fn try_from(value: f32) -> Result<Self, Self::Error> {
        Sample::new(value)
    }
}*/

/* conflicting implementation from core :/
impl TryFrom<Sample> for f32 {
    type Error = Infallible;

    fn try_from(value: Sample) -> Result<Self, Self::Error> {
        Ok(value.0)
    }
}*/

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transparent_layout() {
        assert_eq!(core::mem::size_of::<Sample>(), core::mem::size_of::<f32>());
        assert_eq!(
            core::mem::align_of::<Sample>(),
            core::mem::align_of::<f32>()
        );
    }

    #[test]
    fn sample_can_be_conveniently_constructed() {
        let _ = Sample::new(1.0).unwrap();
        let _ = Sample::new(0.0).unwrap();
        let _ = Sample::new(-1.0).unwrap();
        let _ = Sample::try_from(1.0).unwrap();
        let _ = Sample::try_from(0.0).unwrap();
        let _ = Sample::try_from(-1.0).unwrap();
        let _ = Sample::from(1.0);
        let _ = Sample::from(0.0);
        let _ = Sample::from(-1.0);
        let _: Sample = (0.7_f32).into();
        let _: Sample = (-0.7_f32).into();
    }

    #[test]
    fn sample_rejects_illegal_inputs() {
        assert!(Sample::new(f32::NAN).is_err());
        assert!(Sample::new(f32::INFINITY).is_err());
        assert!(Sample::new(f32::NEG_INFINITY).is_err());
        assert!(Sample::new(1.01).is_err());
        assert!(Sample::new(-1.01).is_err());
    }

    #[test]
    fn sample_behaves_like_f32() {
        let a = Sample::from(0.5);
        let b = Sample::from(1.0);
        assert_eq!(a, 0.5);
        assert_eq!(b, 1.0);
        assert_eq!(a + b, 1.5);
        assert_eq!(a - b, -0.5);
        assert_eq!(a * b, 0.5);
        assert_eq!(a / b, 0.5);
    }

    #[test]
    fn sample_can_be_ordered() {
        let a = Sample::from(0.5);
        let b = Sample::from(1.0);
        assert!(Sample::from(0.5) <= Sample::from(0.5));
        assert!(!(Sample::from(0.5) < Sample::from(0.5)));
        assert!(!(Sample::from(0.5) > Sample::from(0.5)));
        assert!(Sample::from(0.5) < Sample::from(0.6));
        assert!(Sample::from(-0.5) < Sample::from(0.0));
        assert_eq!(Sample::from(-0.0), Sample::from(0.0));

        assert!(Sample::from(0.5) <= 0.5);
        assert!(!(Sample::from(0.5) < 0.5));
        assert!(!(Sample::from(0.5) > 0.5));
        assert!(0.5 < Sample::from(0.6));
        assert!(-0.5 < Sample::from(0.0));
        assert_eq!(0.0, Sample::from(0.0));

        assert!(f32::NAN.partial_cmp(&Sample::from(0.5)).is_none());
        assert!(Sample::from(0.5) < f32::INFINITY);
        assert!(Sample::from(0.5) > f32::NEG_INFINITY);
    }
}
