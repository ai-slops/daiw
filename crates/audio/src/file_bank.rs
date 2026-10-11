use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::file_validation::{validate_digest, validate_file, validate_source_set};
use crate::pcm_format::PCM_HEADER_LEN;
use crate::source::{SourceBank, SourceMetadata, SourceMetadataProvider};
use crate::{AudioError, Snapshot};
use serde::Deserialize;

const MAX_SOURCES: usize = 64;
const MAX_TOTAL_SOURCE_BYTES: u64 = 1024 * 1024 * 1024;
const SOURCE_BLOCK_FRAMES: u64 = 4096;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileRenderInput {
    pub snapshot: Snapshot,
    pub sources: BTreeMap<String, FileSourceDescriptor>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileSourceDescriptor {
    pub cache_id: String,
    pub sha256: String,
    pub frames: u64,
    pub sample_rate: u32,
    pub channels: u16,
    pub byte_length: u64,
}

struct FileSource {
    file: File,
    metadata: SourceMetadata,
    block_start: u64,
    block_frames: u64,
    cache_bytes: Vec<u8>,
}

pub struct FileSourceBank {
    sources: BTreeMap<String, FileSource>,
}

impl FileSourceBank {
    pub fn open(
        root: &Path,
        snapshot: &Snapshot,
        descriptors: &BTreeMap<String, FileSourceDescriptor>,
    ) -> Result<Self, AudioError> {
        if !root.is_absolute() {
            return Err(AudioError::InvalidSourceRoot);
        }
        if descriptors.len() > MAX_SOURCES {
            return Err(AudioError::TooManySources { limit: MAX_SOURCES });
        }
        validate_source_set(snapshot, descriptors)?;
        let canonical_root = root.canonicalize()?;
        let mut total_bytes = 0_u64;
        let mut sources = BTreeMap::new();
        for (asset_id, expected) in descriptors {
            validate_digest(&expected.cache_id)?;
            validate_digest(&expected.sha256)?;
            let path = canonical_root.join(format!("{}.pcm", expected.cache_id));
            let canonical_path = path.canonicalize()?;
            if !canonical_path.starts_with(&canonical_root) {
                return Err(AudioError::SourceEscapesRoot);
            }
            let mut file = File::open(&canonical_path)?;
            let file_len = file.metadata()?.len();
            total_bytes = total_bytes
                .checked_add(file_len)
                .ok_or(AudioError::SourceFilesTooLarge)?;
            if total_bytes > MAX_TOTAL_SOURCE_BYTES {
                return Err(AudioError::SourceFilesTooLarge);
            }
            let metadata = validate_file(&mut file, expected, file_len)?;
            sources.insert(
                asset_id.clone(),
                FileSource {
                    file,
                    metadata,
                    block_start: 0,
                    block_frames: 0,
                    cache_bytes: Vec::new(),
                },
            );
        }
        Ok(Self { sources })
    }
}

pub fn render_file_backed(
    input: FileRenderInput,
    root: &Path,
) -> Result<crate::RenderedAudio, AudioError> {
    let mut bank = FileSourceBank::open(root, &input.snapshot, &input.sources)?;
    crate::render::render_file_sources(&input.snapshot, &mut bank)
}

pub fn render_file_backed_to_wav(
    input: FileRenderInput,
    root: &Path,
    output: &Path,
) -> Result<crate::WavFileDescriptor, AudioError> {
    let mut bank = FileSourceBank::open(root, &input.snapshot, &input.sources)?;
    crate::wav_file::render_to_file(&input.snapshot, &mut bank, output)
}

impl SourceMetadataProvider for FileSourceBank {
    fn metadata(&self, asset_id: &str) -> Result<SourceMetadata, AudioError> {
        self.sources
            .get(asset_id)
            .map(|source| source.metadata)
            .ok_or_else(|| AudioError::MissingAsset {
                asset_id: asset_id.to_owned(),
            })
    }
}

impl SourceBank for FileSourceBank {
    fn read_frame(
        &mut self,
        asset_id: &str,
        frame: u64,
        output: &mut [f32; 8],
    ) -> Result<bool, AudioError> {
        let source = self
            .sources
            .get_mut(asset_id)
            .ok_or_else(|| AudioError::MissingAsset {
                asset_id: asset_id.to_owned(),
            })?;
        if frame >= source.metadata.frames {
            return Ok(false);
        }
        if frame < source.block_start || frame >= source.block_start + source.block_frames {
            load_block(source, frame)?;
        }
        let frame_bytes = usize::from(source.metadata.channels) * 4;
        let offset = usize::try_from(frame - source.block_start)?
            .checked_mul(frame_bytes)
            .ok_or(AudioError::InvalidFrameCount)?;
        let encoded = source
            .cache_bytes
            .get(offset..offset + frame_bytes)
            .ok_or(AudioError::InvalidPcmLength)?;
        for (target, sample) in output.iter_mut().zip(encoded.chunks_exact(4)) {
            *target = f32::from_le_bytes(sample.try_into()?);
        }
        Ok(true)
    }
}

fn load_block(source: &mut FileSource, frame: u64) -> Result<(), AudioError> {
    let block_start = frame / SOURCE_BLOCK_FRAMES * SOURCE_BLOCK_FRAMES;
    let block_frames = SOURCE_BLOCK_FRAMES.min(source.metadata.frames - block_start);
    let channels = usize::from(source.metadata.channels);
    let block_bytes = usize::try_from(block_frames)?
        .checked_mul(channels)
        .and_then(|samples| samples.checked_mul(4))
        .ok_or(AudioError::InvalidFrameCount)?;
    source.cache_bytes.resize(block_bytes, 0);
    let byte_offset = u64::try_from(PCM_HEADER_LEN)?
        .checked_add(block_start * u64::from(source.metadata.channels) * 4)
        .ok_or(AudioError::InvalidFrameCount)?;
    source.file.seek(SeekFrom::Start(byte_offset))?;
    source.file.read_exact(&mut source.cache_bytes)?;
    for encoded in source.cache_bytes.chunks_exact(4) {
        if !f32::from_le_bytes(encoded.try_into()?).is_finite() {
            return Err(AudioError::NonFiniteDecodedPcm);
        }
    }
    source.block_start = block_start;
    source.block_frames = block_frames;
    Ok(())
}

#[cfg(test)]
#[path = "file_bank_tests.rs"]
mod tests;
