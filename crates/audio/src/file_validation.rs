use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use sha2::{Digest, Sha256};

use crate::file_bank::FileSourceDescriptor;
use crate::pcm_format::{PCM_HEADER_LEN, PcmHeader, lowercase_hex};
use crate::source::SourceMetadata;
use crate::{AudioError, Snapshot};

pub(crate) fn validate_source_set(
    snapshot: &Snapshot,
    descriptors: &BTreeMap<String, FileSourceDescriptor>,
) -> Result<(), AudioError> {
    let required: BTreeSet<&str> = snapshot
        .regions
        .iter()
        .map(|region| region.asset_id.as_str())
        .collect();
    for asset_id in &required {
        if !descriptors.contains_key(*asset_id) {
            return Err(AudioError::MissingAsset {
                asset_id: (*asset_id).to_owned(),
            });
        }
    }
    if let Some(extra) = descriptors
        .keys()
        .find(|id| !required.contains(id.as_str()))
    {
        return Err(AudioError::ExtraSource {
            asset_id: extra.clone(),
        });
    }
    Ok(())
}

pub(crate) fn validate_digest(value: &str) -> Result<(), AudioError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(AudioError::InvalidSha256)
    }
}

pub(crate) fn validate_file(
    file: &mut File,
    expected: &FileSourceDescriptor,
    file_len: u64,
) -> Result<SourceMetadata, AudioError> {
    let mut header_bytes = [0_u8; PCM_HEADER_LEN];
    file.read_exact(&mut header_bytes)?;
    let header = PcmHeader::decode(&header_bytes)?;
    if file_len != header.byte_length()? || file_len != expected.byte_length {
        return Err(AudioError::InvalidPcmLength);
    }
    if header.frames != expected.frames
        || header.sample_rate != expected.sample_rate
        || header.channels != expected.channels
    {
        return Err(AudioError::PcmMetadataMismatch);
    }
    let mut whole_hasher = Sha256::new();
    whole_hasher.update(header_bytes);
    let mut payload_hasher = Sha256::new();
    let mut remaining = header.payload_bytes;
    let block_bytes = usize::from(header.channels) * 4096 * 4;
    let mut bytes = vec![0_u8; block_bytes];
    while remaining > 0 {
        let count = usize::try_from(remaining.min(u64::try_from(bytes.len())?))?;
        file.read_exact(&mut bytes[..count])?;
        whole_hasher.update(&bytes[..count]);
        payload_hasher.update(&bytes[..count]);
        for encoded in bytes[..count].chunks_exact(4) {
            if !f32::from_le_bytes(encoded.try_into()?).is_finite() {
                return Err(AudioError::NonFiniteDecodedPcm);
            }
        }
        remaining -= u64::try_from(count)?;
    }
    if payload_hasher.finalize().as_slice() != header.payload_sha256 {
        return Err(AudioError::PcmPayloadHashMismatch);
    }
    if lowercase_hex(&whole_hasher.finalize()) != expected.sha256 {
        return Err(AudioError::PcmFileHashMismatch);
    }
    file.seek(SeekFrom::Start(u64::try_from(PCM_HEADER_LEN)?))?;
    Ok(SourceMetadata {
        sample_rate: header.sample_rate,
        channels: header.channels,
        frames: header.frames,
    })
}
