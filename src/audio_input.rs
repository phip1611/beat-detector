#[derive(Debug)]
pub enum AudioInput<'a> {
    /// The audio input stream only consists of mono samples.
    Mono(&'a [f32]),
    /// The audio input streams consists of interleaved samples following a
    /// LRLRLR scheme. This is typically the case for stereo channel audio.
    InterleavedLR(&'a [f32]),
}

/*impl<'a> AudioInput<'a> {
    pub fn iter(&self) -> &dyn Iterator<Item = &f32> {
        match self {
            AudioInput::Mono(samples) => &samples.iter(),
            AudioInput::InterleavedLR(samples) => &samples.chunks(2).map(|lr| (lr[0] + lr[1]) / 2.0)
        }
    }
}*/

/*impl Iterator for AudioInput {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}*/

#[cfg(test)]
mod tests {

    // fn stereo_to:
}
