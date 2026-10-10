use std::io::Read;
use std::path::Path;

use daiw_audio::{AudioError, WavFileDescriptor, WavStreamFile, validate_wav_file};

pub fn validate(
    path: &Path,
    sha256: &str,
) -> Result<(WavStreamFile, WavFileDescriptor), AudioError> {
    let mut file = std::fs::File::open(path)?;
    let byte_length = file.metadata()?.len();
    let mut header = [0_u8; 44];
    file.read_exact(&mut header)?;
    let payload_bytes = u32::from_le_bytes(header[40..44].try_into()?);
    let frames = usize::try_from(payload_bytes / 4)?;
    let descriptor = WavFileDescriptor {
        format: "daiw-wav-pcm16-v1",
        sample_rate: 48_000,
        channels: 2,
        frames,
        byte_length,
        sha256: sha256.to_owned(),
    };
    let stream = validate_wav_file(path, &descriptor)?;
    Ok((stream, descriptor))
}
