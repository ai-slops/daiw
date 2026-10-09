use crate::{AudioError, RenderedAudio};

pub fn wav(audio: &RenderedAudio) -> Result<Vec<u8>, AudioError> {
    crate::validate::finite_samples(&audio.samples, "rendered audio")?;
    let payload_len = audio
        .samples
        .len()
        .checked_mul(2)
        .and_then(|length| u32::try_from(length).ok())
        .ok_or(AudioError::WavTooLarge)?;
    let header = header_with_payload_len(audio.sample_rate, audio.channels, payload_len)?;
    let capacity = usize::try_from(payload_len)
        .ok()
        .and_then(|length| length.checked_add(44))
        .ok_or(AudioError::WavTooLarge)?;
    let mut output = Vec::new();
    output.try_reserve_exact(capacity)?;
    output.extend_from_slice(&header);
    for sample in &audio.samples {
        let pcm = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
        output.extend_from_slice(&pcm.to_le_bytes());
    }
    Ok(output)
}

pub(crate) fn wav_header(
    sample_rate: u32,
    channels: u16,
    frames: usize,
) -> Result<[u8; 44], AudioError> {
    let payload_len = frames
        .checked_mul(usize::from(channels))
        .and_then(|samples| samples.checked_mul(2))
        .and_then(|length| u32::try_from(length).ok())
        .ok_or(AudioError::WavTooLarge)?;
    header_with_payload_len(sample_rate, channels, payload_len)
}

fn header_with_payload_len(
    sample_rate: u32,
    channels: u16,
    payload_len: u32,
) -> Result<[u8; 44], AudioError> {
    let riff_len = 36_u32
        .checked_add(payload_len)
        .ok_or(AudioError::WavTooLarge)?;
    let byte_rate = sample_rate
        .checked_mul(u32::from(channels))
        .and_then(|rate| rate.checked_mul(2))
        .ok_or(AudioError::WavTooLarge)?;
    let block_align = channels.checked_mul(2).ok_or(AudioError::WavTooLarge)?;
    let mut header = [0_u8; 44];
    header[0..4].copy_from_slice(b"RIFF");
    header[4..8].copy_from_slice(&riff_len.to_le_bytes());
    header[8..16].copy_from_slice(b"WAVEfmt ");
    header[16..20].copy_from_slice(&16_u32.to_le_bytes());
    header[20..22].copy_from_slice(&1_u16.to_le_bytes());
    header[22..24].copy_from_slice(&channels.to_le_bytes());
    header[24..28].copy_from_slice(&sample_rate.to_le_bytes());
    header[28..32].copy_from_slice(&byte_rate.to_le_bytes());
    header[32..34].copy_from_slice(&block_align.to_le_bytes());
    header[34..36].copy_from_slice(&16_u16.to_le_bytes());
    header[36..40].copy_from_slice(b"data");
    header[40..44].copy_from_slice(&payload_len.to_le_bytes());
    Ok(header)
}
