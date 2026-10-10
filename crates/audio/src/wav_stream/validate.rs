use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use sha2::{Digest, Sha256};

use super::types::descriptor_frames;
use super::{MAX_WAV_STREAM_FRAMES, Sha256Digest, WavStreamError, WavStreamFile};
use crate::WavFileDescriptor;

const HEADER_LEN: usize = 44;
const SCAN_BYTES: usize = 16 * 1024;

pub fn validate_wav_file(
    path: &Path,
    descriptor: &WavFileDescriptor,
) -> Result<WavStreamFile, WavStreamError> {
    if !path.is_absolute() {
        return Err(WavStreamError::RelativePath);
    }
    let digest = Sha256Digest::parse(&descriptor.sha256)?;
    let frames = descriptor_frames(descriptor)?;
    validate_descriptor(descriptor, frames)?;
    let mut file = OpenOptions::new().read(true).open(path)?;
    let length = file.metadata()?.len();
    if length != descriptor.byte_length {
        return Err(WavStreamError::DescriptorMismatch);
    }
    let mut header = [0_u8; HEADER_LEN];
    file.read_exact(&mut header)?;
    validate_header(&header, frames)?;
    file.seek(SeekFrom::Start(0))?;
    validate_hash(&mut file, digest)?;
    file.seek(SeekFrom::Start(HEADER_LEN as u64))?;
    Ok(WavStreamFile { file, frames })
}

fn validate_descriptor(descriptor: &WavFileDescriptor, frames: u32) -> Result<(), WavStreamError> {
    if frames > MAX_WAV_STREAM_FRAMES {
        return Err(WavStreamError::TooManyFrames);
    }
    let expected_length = u64::from(frames)
        .checked_mul(4)
        .and_then(|payload| payload.checked_add(HEADER_LEN as u64))
        .ok_or(WavStreamError::TooManyFrames)?;
    if descriptor.format != "daiw-wav-pcm16-v1"
        || descriptor.sample_rate != 48_000
        || descriptor.channels != 2
        || descriptor.byte_length != expected_length
    {
        return Err(WavStreamError::DescriptorMismatch);
    }
    Ok(())
}

fn validate_header(header: &[u8; HEADER_LEN], frames: u32) -> Result<(), WavStreamError> {
    let payload = frames.checked_mul(4).ok_or(WavStreamError::TooManyFrames)?;
    let riff = payload
        .checked_add(36)
        .ok_or(WavStreamError::TooManyFrames)?;
    let mut expected = [0_u8; HEADER_LEN];
    expected[0..4].copy_from_slice(b"RIFF");
    expected[4..8].copy_from_slice(&riff.to_le_bytes());
    expected[8..16].copy_from_slice(b"WAVEfmt ");
    expected[16..20].copy_from_slice(&16_u32.to_le_bytes());
    expected[20..22].copy_from_slice(&1_u16.to_le_bytes());
    expected[22..24].copy_from_slice(&2_u16.to_le_bytes());
    expected[24..28].copy_from_slice(&48_000_u32.to_le_bytes());
    expected[28..32].copy_from_slice(&192_000_u32.to_le_bytes());
    expected[32..34].copy_from_slice(&4_u16.to_le_bytes());
    expected[34..36].copy_from_slice(&16_u16.to_le_bytes());
    expected[36..40].copy_from_slice(b"data");
    expected[40..44].copy_from_slice(&payload.to_le_bytes());
    if *header != expected {
        return Err(WavStreamError::InvalidHeader);
    }
    Ok(())
}

fn validate_hash(file: &mut File, expected: Sha256Digest) -> Result<(), WavStreamError> {
    let mut hasher = Sha256::new();
    let mut bytes = [0_u8; SCAN_BYTES];
    loop {
        let count = file.read(&mut bytes)?;
        if count == 0 {
            break;
        }
        hasher.update(&bytes[..count]);
    }
    if hasher.finalize().as_slice() != expected.0 {
        return Err(WavStreamError::DescriptorMismatch);
    }
    Ok(())
}
