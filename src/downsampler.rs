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

/// Downsampler that buffers `n` elements and emits the mean value every `n`
/// invocations.
#[derive(Debug)]
pub struct Downsampler {
    sum: i64,
    capacity: usize,
    elem_count: usize,
}

impl Downsampler {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self {
            capacity,
            elem_count: 0,
            sum: 0,
        }
    }

    #[inline]
    pub fn consume(&mut self, sample: i16) -> Option<i16> {
        self.sum += sample as i64;
        self.elem_count += 1;

        if self.elem_count == self.capacity {
            let avg = self.sum / self.elem_count as i64;
            self.sum = 0;
            self.elem_count = 0;
            Some(avg as i16)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_downsampler() {
        let mut downsampler = Downsampler::new(10);

        let samples = [1, 2, 3, 4, 5, 6, 7, 8, 9];
        for sample in samples {
            let res = downsampler.consume(sample);
            assert!(res == None)
        }

        assert!(downsampler.consume(10) == Some(5));

        assert!(downsampler.consume(7) == None);

        let samples = [7; 8];
        for sample in samples {
            let res = downsampler.consume(sample);
            assert!(res == None)
        }

        assert!(downsampler.consume(7) == Some(7));
    }
}
