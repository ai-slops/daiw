use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{FileSource, FileSourceBank, SOURCE_BLOCK_FRAMES};
use crate::source::{SourceBank, SourceMetadata};

static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

fn source_fixture() -> Result<(FileSourceBank, PathBuf), crate::AudioError> {
    let channels = 8_u16;
    let frames = SOURCE_BLOCK_FRAMES + 1;
    let path = std::env::temp_dir().join(format!(
        "daiw-block-cache-{}-{}",
        std::process::id(),
        NEXT_FILE.fetch_add(1, Ordering::Relaxed)
    ));
    let mut fixture = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    fixture.write_all(&[0_u8; crate::PCM_HEADER_LEN])?;
    for frame in 0..frames {
        for channel in 0..channels {
            fixture.write_all(&(frame as f32 + f32::from(channel) / 10.0).to_le_bytes())?;
        }
    }
    drop(fixture);
    let file = OpenOptions::new().read(true).open(&path)?;
    let source = FileSource {
        file,
        metadata: SourceMetadata {
            sample_rate: 48_000,
            channels,
            frames,
        },
        block_start: 0,
        block_frames: 0,
        cache_bytes: Vec::new(),
    };
    Ok((
        FileSourceBank {
            sources: BTreeMap::from([("asset".into(), source)]),
        },
        path,
    ))
}

#[test]
fn encoded_source_cache_is_bounded_to_128_kib() -> Result<(), crate::AudioError> {
    // Given
    let (mut bank, path) = source_fixture()?;
    let mut output = [0.0_f32; 8];

    // When
    bank.read_frame("asset", 0, &mut output)?;

    // Then
    let source = bank
        .sources
        .get("asset")
        .ok_or(crate::AudioError::NoAudioTrack)?;
    assert_eq!(source.cache_bytes.len(), 4096 * 8 * 4);
    assert!(source.cache_bytes.len() <= 128 * 1024);
    assert_eq!(output, [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7]);
    std::fs::remove_file(path)?;
    Ok(())
}

#[test]
fn encoded_source_cache_reads_across_block_boundary() -> Result<(), crate::AudioError> {
    // Given
    let (mut bank, path) = source_fixture()?;
    let mut output = [0.0_f32; 8];

    // When
    bank.read_frame("asset", SOURCE_BLOCK_FRAMES, &mut output)?;

    // Then
    let source = bank
        .sources
        .get("asset")
        .ok_or(crate::AudioError::NoAudioTrack)?;
    assert_eq!(source.block_start, SOURCE_BLOCK_FRAMES);
    assert_eq!(source.cache_bytes.len(), 8 * 4);
    assert_eq!(output[0], 4096.0);
    assert_eq!(output[7], 4096.7);
    std::fs::remove_file(path)?;
    Ok(())
}
