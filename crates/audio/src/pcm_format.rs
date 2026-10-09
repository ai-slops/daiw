use serde::Serialize;

use crate::AudioError;

pub const PCM_MAGIC: &[u8; 8] = b"DAIWPCM\0";
pub const PCM_HEADER_LEN: usize = 80;
pub const PCM_VERSION: u16 = 1;
pub const PCM_SAMPLE_FORMAT_F32_LE: u16 = 1;
pub const PCM_PEAK_COUNT: usize = 128;
pub const MAX_PCM_PAYLOAD_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_PCM_DURATION_SECONDS: u64 = 180;
pub const MIN_PCM_SAMPLE_RATE: u32 = 8_000;
pub const MAX_PCM_SAMPLE_RATE: u32 = 192_000;
pub const MAX_PCM_CHANNELS: u16 = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PcmHeader {
    pub sample_rate: u32,
    pub channels: u16,
    pub frames: u64,
    pub payload_bytes: u64,
    pub payload_sha256: [u8; 32],
}

impl PcmHeader {
    pub fn encode(&self) -> [u8; PCM_HEADER_LEN] {
        let mut bytes = [0_u8; PCM_HEADER_LEN];
        bytes[0..8].copy_from_slice(PCM_MAGIC);
        bytes[8..10].copy_from_slice(&PCM_VERSION.to_le_bytes());
        bytes[10..12].copy_from_slice(&(PCM_HEADER_LEN as u16).to_le_bytes());
        bytes[12..16].copy_from_slice(&self.sample_rate.to_le_bytes());
        bytes[16..18].copy_from_slice(&self.channels.to_le_bytes());
        bytes[18..20].copy_from_slice(&PCM_SAMPLE_FORMAT_F32_LE.to_le_bytes());
        bytes[20..28].copy_from_slice(&self.frames.to_le_bytes());
        bytes[28..36].copy_from_slice(&self.payload_bytes.to_le_bytes());
        bytes[36..68].copy_from_slice(&self.payload_sha256);
        bytes
    }

    pub fn decode(bytes: &[u8; PCM_HEADER_LEN]) -> Result<Self, AudioError> {
        if &bytes[0..8] != PCM_MAGIC
            || u16::from_le_bytes([bytes[8], bytes[9]]) != PCM_VERSION
            || u16::from_le_bytes([bytes[10], bytes[11]]) != PCM_HEADER_LEN as u16
            || u16::from_le_bytes([bytes[18], bytes[19]]) != PCM_SAMPLE_FORMAT_F32_LE
            || bytes[68..80].iter().any(|byte| *byte != 0)
        {
            return Err(AudioError::InvalidPcmHeader);
        }
        let sample_rate = u32::from_le_bytes(bytes[12..16].try_into()?);
        let channels = u16::from_le_bytes(bytes[16..18].try_into()?);
        let frames = u64::from_le_bytes(bytes[20..28].try_into()?);
        let payload_bytes = u64::from_le_bytes(bytes[28..36].try_into()?);
        let payload_sha256 = bytes[36..68].try_into()?;
        validate_metadata(sample_rate, channels, frames, payload_bytes)?;
        Ok(Self {
            sample_rate,
            channels,
            frames,
            payload_bytes,
            payload_sha256,
        })
    }

    pub fn byte_length(&self) -> Result<u64, AudioError> {
        (PCM_HEADER_LEN as u64)
            .checked_add(self.payload_bytes)
            .ok_or(AudioError::PcmPayloadTooLarge)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PcmDescriptor {
    pub format: &'static str,
    pub frames: u64,
    pub sample_rate: u32,
    pub channels: u16,
    pub byte_length: u64,
    pub sha256: String,
    pub peaks: Vec<f32>,
}

pub fn validate_metadata(
    sample_rate: u32,
    channels: u16,
    frames: u64,
    payload_bytes: u64,
) -> Result<(), AudioError> {
    if !(MIN_PCM_SAMPLE_RATE..=MAX_PCM_SAMPLE_RATE).contains(&sample_rate) {
        return Err(AudioError::UnsupportedPcmSampleRate { sample_rate });
    }
    if !(1..=MAX_PCM_CHANNELS).contains(&channels) {
        return Err(AudioError::UnsupportedPcmChannels { channels });
    }
    let expected = frames
        .checked_mul(u64::from(channels))
        .and_then(|samples| samples.checked_mul(4))
        .ok_or(AudioError::PcmPayloadTooLarge)?;
    if payload_bytes != expected || payload_bytes > MAX_PCM_PAYLOAD_BYTES {
        return Err(AudioError::PcmPayloadTooLarge);
    }
    if frames > u64::from(sample_rate) * MAX_PCM_DURATION_SECONDS {
        return Err(AudioError::PcmDurationTooLong);
    }
    Ok(())
}

#[must_use]
pub fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}
