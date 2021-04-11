//! Module for struct "WindowStats", i.e. analysis on sample windows.

/// Holds information about the (original) audio window/data
/// e.g. the maximum amplitude.
#[derive(Debug)]
pub struct WindowStats {
    // the maximum amplitude inside a signed 16 bit sampled audio data window
    max: u16,
}

impl WindowStats {
    #[inline(always)]
    pub fn max(&self) -> u16 {
        self.max
    }
}

impl From<&[i16]> for WindowStats {

    #[inline(always)]
    fn from(samples: &[i16]) -> Self {
        let mut abs_samples_ordered = samples.iter()
            // to prevent any overflow in next step
            .map(|x| if *x == i16::MIN { x + 1 } else { *x })
            .map(|x| x.abs())
            .collect::<Vec<_>>();
        abs_samples_ordered.sort();
        let max = *abs_samples_ordered.last().unwrap() as u16;

        Self {
            max
        }
    }
}
