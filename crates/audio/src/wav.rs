use crate::{AudioError, RenderedAudio};

pub fn wav(audio: &RenderedAudio) -> Result<Vec<u8>, AudioError> {
    crate::validate::finite_samples(&audio.samples, "rendered audio")?;
    let payload_len = audio
        .samples
        .len()
        .checked_mul(2)
        .and_then(|length| u32::try_from(length).ok())
        .ok_or(AudioError::WavTooLarge)?;
    let riff_len = 36_u32
        .checked_add(payload_len)
        .ok_or(AudioError::WavTooLarge)?;
    let byte_rate = audio
        .sample_rate
        .checked_mul(u32::from(audio.channels))
        .and_then(|rate| rate.checked_mul(2))
        .ok_or(AudioError::WavTooLarge)?;
    let block_align = audio
        .channels
        .checked_mul(2)
        .ok_or(AudioError::WavTooLarge)?;
    let capacity = usize::try_from(payload_len)
        .ok()
        .and_then(|length| length.checked_add(44))
        .ok_or(AudioError::WavTooLarge)?;
    let mut output = Vec::new();
    output.try_reserve_exact(capacity)?;
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&riff_len.to_le_bytes());
    output.extend_from_slice(b"WAVEfmt ");
    output.extend_from_slice(&16_u32.to_le_bytes());
    output.extend_from_slice(&1_u16.to_le_bytes());
    output.extend_from_slice(&audio.channels.to_le_bytes());
    output.extend_from_slice(&audio.sample_rate.to_le_bytes());
    output.extend_from_slice(&byte_rate.to_le_bytes());
    output.extend_from_slice(&block_align.to_le_bytes());
    output.extend_from_slice(&16_u16.to_le_bytes());
    output.extend_from_slice(b"data");
    output.extend_from_slice(&payload_len.to_le_bytes());
    for sample in &audio.samples {
        let pcm = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
        output.extend_from_slice(&pcm.to_le_bytes());
    }
    Ok(output)
}
