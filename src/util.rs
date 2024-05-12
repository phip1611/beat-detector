//! Some common utilities required internally but also useful for external
//! users, when working with this library.

/// Convenient wrapper around [`i16_sample_to_f32`] that works on
/// stream of samples.
pub fn i16_samples_to_f32(samples: impl Iterator<Item = i16>) -> impl Iterator<Item = f32> {
    samples.map(i16_sample_to_f32)
}

/// Convenient wrapper around [`f32_sample_to_i16`] that works on
/// stream of samples.
pub fn f32_samples_to_i16(samples: impl Iterator<Item = f32>) -> impl Iterator<Item = i16> {
    samples.map(f32_sample_to_i16)
}

/// Transforms an audio sample in range `i16::MIN..=i16::MAX` to a `f32` in
/// range `-1.0..1.0`.
pub fn i16_sample_to_f32(mut val: i16) -> f32 {
    if val == i16::MIN {
        val += 1;
    }
    val as f32 / i16::MAX as f32
}

/// Transforms an audio sample of type `f32` in range `-1.0..1.0` to  a `i16` in
/// range `-i16::MAX..=i16::MAX`.
pub fn f32_sample_to_i16(val: f32) -> i16 {
    (val * i16::MAX as f32) as i16
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

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
        check!(f32_sample_to_i16(0.0) == 0);
        check!(f32_sample_to_i16(-0.5) == -i16::MAX / 2);
        check!(f32_sample_to_i16(0.5) == i16::MAX / 2);
        check!(f32_sample_to_i16(-1.0) == -i16::MAX);
        check!(f32_sample_to_i16(1.0) == i16::MAX);
    }

    #[test]
    fn test_i16_samples_to_f32() {
        check!(
            &i16_samples_to_f32([i16::MIN, 0, i16::MAX].into_iter()).collect::<Vec<_>>()
                == &[-1.0, 0.0, 1.0]
        );
    }

    #[test]
    fn test_f32_samples_to_i16() {
        check!(
            &f32_samples_to_i16([-1.0, 0.0, 1.0].into_iter()).collect::<Vec<_>>()
                == &[-i16::MAX, 0, i16::MAX]
        );
    }
}
