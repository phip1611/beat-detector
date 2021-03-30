use lowpass_filter as lpf;

pub fn is_beat(samples: &[i16], sampling_rate: u16) -> bool {
    const CUTOFF_FR: u16 = 100;
    const THRESHOLD: i16 = (i16::MAX as f32 * 0.5) as i16;
    let mut samples = samples.to_vec();
    lpf::simple::sp::apply_lpf_i16_sp(
        &mut samples,
        sampling_rate,
        CUTOFF_FR,
    );
    samples.iter().any(|s| s.abs() >= THRESHOLD)
}
