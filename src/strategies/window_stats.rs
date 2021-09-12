/*
MIT License

Copyright (c) 2021 Philipp Schuster

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
    pub const fn max(&self) -> u16 {
        self.max
    }
}

impl From<&[i16]> for WindowStats {
    #[inline(always)]
    fn from(samples: &[i16]) -> Self {
        let mut abs_samples_ordered = samples
            .iter()
            // to prevent any overflow in next step
            .map(|x| if *x == i16::MIN { x + 1 } else { *x })
            .map(|x| x.abs())
            .collect::<Vec<_>>();
        abs_samples_ordered.sort_unstable();
        let max = *abs_samples_ordered.last().unwrap() as u16;

        Self { max }
    }
}
