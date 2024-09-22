//! Some common utilities required internally but also useful for external
//! users, when working with this library.

/// Transforms an audio sample in range `i16::MIN..=i16::MAX` to a `f32` in
/// range `-1.0..1.0`.
#[inline]
pub fn i16_sample_to_f32(val: i16) -> f32 {
    // If to prevent division result >1.0.
    if val == i16::MIN {
        -1.0
    } else {
        val as f32 / i16::MAX as f32
    }
}

/// The sample is out of range `-1.0..1.0`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct OutOfRangeError(f32);

/// Transforms an audio sample of type `f32` in range `-1.0..1.0` to  a `i16` in
/// range `-i16::MAX..=i16::MAX`.
#[inline]
pub fn f32_sample_to_i16(val: f32) -> Result<i16, OutOfRangeError> {
    if val.is_finite() && libm::fabsf(val) <= 1.0 {
        Ok((val * i16::MAX as f32) as i16)
    } else {
        Err(OutOfRangeError(val))
    }
}

/// Transforms two stereo samples (that reflect the same point in time on
/// different channels) into one mono sample.
#[inline]
pub const fn stereo_to_mono(l: i16, r: i16) -> i16 {
    let l = l as i32;
    let r = r as i32;
    let avg = (l + r) / 2;
    avg as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_i16_sample_to_f32() {
        check!(i16_sample_to_f32(0) == 0.0);
        check!(approx_eq!(
            f32,
            i16_sample_to_f32(i16::MAX / 2),
            0.5,
            epsilon = 0.01
        ));
        check!(i16_sample_to_f32(i16::MAX) == 1.0);
        check!(i16_sample_to_f32(-i16::MAX) == -1.0);
        check!(i16_sample_to_f32(i16::MIN) == -1.0);
    }

    #[test]
    fn test_f32_sample_to_i16() {
        check!(f32_sample_to_i16(0.0) == Ok(0));
        check!(f32_sample_to_i16(-0.5) == Ok(-i16::MAX / 2));
        check!(f32_sample_to_i16(0.5) == Ok(i16::MAX / 2));
        check!(f32_sample_to_i16(-1.0) == Ok(-i16::MAX));
        check!(f32_sample_to_i16(1.0) == Ok(i16::MAX));
        check!(f32_sample_to_i16(1.1) == Err(OutOfRangeError(1.1)));
        check!(matches!(
            f32_sample_to_i16(f32::NAN),
            Err(OutOfRangeError(_))
        ));
    }
}
