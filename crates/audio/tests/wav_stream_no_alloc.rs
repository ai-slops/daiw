use std::alloc::System;

use daiw_audio::{FramePosition, WavFileDescriptor, WavStreamOptions, validate_wav_file};
use sha2::{Digest, Sha256};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};

#[path = "support/must.rs"]
mod support;
#[path = "support/temp_dir.rs"]
mod temp_dir;
use support::must;
use temp_dir::TempDir;

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

#[test]
fn callback_paths_allocate_and_deallocate_nothing() {
    // Given
    let temp = must(TempDir::new("wav-stream-no-alloc"));
    let path = temp.path().join("input.wav");
    let samples = [[1_i16, -1_i16], [2, -2]];
    let mut bytes = Vec::with_capacity(52);
    bytes.extend_from_slice(b"RIFF\x2c\0\0\0WAVEfmt \x10\0\0\0\x01\0\x02\0\x80\xbb\0\0\0\xee\x02\0\x04\0\x10\0data\x08\0\0\0");
    for frame in samples {
        bytes.extend_from_slice(&frame[0].to_le_bytes());
        bytes.extend_from_slice(&frame[1].to_le_bytes());
    }
    must(std::fs::write(&path, &bytes));
    let descriptor = WavFileDescriptor {
        format: "daiw-wav-pcm16-v1",
        sample_rate: 48_000,
        channels: 2,
        frames: 2,
        byte_length: 52,
        sha256: format!("{:x}", Sha256::digest(&bytes)),
    };
    let (mut control, mut callback) =
        must(must(validate_wav_file(&path, &descriptor)).open(WavStreamOptions::default()));
    let mut output = [1.0_f32; 8];

    // When
    let region = Region::new(GLOBAL);
    callback.fill_output(&mut output);
    must(control.play());
    callback.fill_output(&mut output);
    let change = region.change();

    // Then
    assert_eq!(change.allocations, 0);
    assert_eq!(change.deallocations, 0);
    assert_eq!(
        &output[..4],
        &[
            1.0 / 32_768.0,
            -1.0 / 32_768.0,
            2.0 / 32_768.0,
            -2.0 / 32_768.0
        ]
    );

    must(control.seek(FramePosition::new(1)));
    let stale_region = Region::new(GLOBAL);
    callback.fill_output(&mut output);
    let stale_change = stale_region.change();
    assert_eq!(stale_change.allocations, 0);
    assert_eq!(stale_change.deallocations, 0);
    assert!(control.stats().stale_blocks > 0);
    must(control.shutdown());
}
