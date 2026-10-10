use daiw_audio::{
    FrameLoopRange, FramePosition, STREAM_BLOCK_FRAMES, WavFileDescriptor, WavStreamError,
    WavStreamOptions, WavStreamState, validate_wav_file,
};
use sha2::{Digest, Sha256};

#[path = "support/must.rs"]
mod support;
#[path = "support/temp_dir.rs"]
mod temp_dir;
use support::must;
use temp_dir::TempDir;

fn fixture(label: &str, frames: usize) -> (TempDir, std::path::PathBuf, WavFileDescriptor) {
    let temp = must(TempDir::new(label));
    let path = temp.path().join("input.wav");
    let payload_len = must(u32::try_from(frames * 4));
    let mut bytes = Vec::with_capacity(44 + frames * 4);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + payload_len).to_le_bytes());
    bytes.extend_from_slice(
        b"WAVEfmt \x10\0\0\0\x01\0\x02\0\x80\xbb\0\0\0\xee\x02\0\x04\0\x10\0data",
    );
    bytes.extend_from_slice(&payload_len.to_le_bytes());
    for frame in 0..frames {
        let sample = must(i16::try_from(frame % 32_000));
        bytes.extend_from_slice(&sample.to_le_bytes());
        bytes.extend_from_slice(&(-sample).to_le_bytes());
    }
    must(std::fs::write(&path, &bytes));
    let descriptor = WavFileDescriptor {
        format: "daiw-wav-pcm16-v1",
        sample_rate: 48_000,
        channels: 2,
        frames,
        byte_length: must(u64::try_from(bytes.len())),
        sha256: format!("{:x}", Sha256::digest(&bytes)),
    };
    (temp, path, descriptor)
}

#[test]
fn partial_callbacks_cross_block_boundary_without_skipping() {
    // Given
    let (_temp, path, descriptor) = fixture("wav-stream-crossing", 4_097);
    let (mut control, mut callback) =
        must(must(validate_wav_file(&path, &descriptor)).open(WavStreamOptions::default()));
    must(control.play());
    let mut first = [0.0_f32; 8_190];
    let mut second = [0.0_f32; 4];
    callback.fill_output(&mut first);

    // When
    callback.fill_output(&mut second);

    // Then
    assert_eq!(first[8_188], 4_094.0 / 32_768.0);
    assert_eq!(
        second,
        [
            4_095.0 / 32_768.0,
            -4_095.0 / 32_768.0,
            4_096.0 / 32_768.0,
            -4_096.0 / 32_768.0
        ]
    );
    assert_eq!(control.position(), FramePosition::new(4_097));
    assert_eq!(control.state(), WavStreamState::Ended);
    assert_eq!(control.stats().underrun_callbacks, 0);
    must(control.shutdown());
}

#[test]
fn seek_stop_and_loop_changes_publish_new_positions() {
    // Given
    let (_temp, path, descriptor) = fixture("wav-stream-controls", 16);
    let (mut control, mut callback) =
        must(must(validate_wav_file(&path, &descriptor)).open(WavStreamOptions::default()));
    let range = must(FrameLoopRange::new(
        FramePosition::new(4),
        FramePosition::new(8),
        16,
    ));
    must(control.set_loop(Some(range)));
    must(control.seek(FramePosition::new(6)));
    must(control.play());
    let mut output = [0.0_f32; 8];
    callback.fill_output(&mut output);

    // When
    must(control.stop());

    // Then
    assert_eq!(control.position(), FramePosition::new(0));
    assert_eq!(control.state(), WavStreamState::Paused);
    assert!(output.iter().all(|sample| sample.is_finite()));
    let mut silence = [1.0_f32; 3];
    callback.fill_output(&mut silence);
    assert_eq!(silence, [0.0; 3]);
    assert!(control.stats().stale_blocks > 0);
    must(control.shutdown());
}

#[test]
fn read_failure_is_typed_and_shutdown_joins_a_full_loop_queue() {
    // Given
    let (_temp, path, descriptor) = fixture("wav-stream-read-failure", 1);
    let validated = must(validate_wav_file(&path, &descriptor));
    must(must(std::fs::OpenOptions::new().write(true).open(&path)).set_len(44));

    // When
    let failed = validated.open(WavStreamOptions::default());

    // Then
    assert!(failed.is_err());

    let (_temp, path, descriptor) = fixture("wav-stream-full-shutdown", 2);
    let range = must(FrameLoopRange::new(
        FramePosition::new(0),
        FramePosition::new(2),
        2,
    ));
    let options = WavStreamOptions {
        start_frame: FramePosition::new(0),
        loop_range: Some(range),
    };
    let (mut control, _callback) = must(must(validate_wav_file(&path, &descriptor)).open(options));
    let reporter = control.device_error_reporter();
    reporter.record();
    reporter.clone().record();
    assert_eq!(control.stats().device_errors, 2);
    must(control.shutdown());
    assert_eq!(control.state(), WavStreamState::Shutdown);
    assert!(control.stop().is_err());
    assert!(control.play().is_err());
    assert!(control.pause().is_err());
}

#[test]
fn start_and_seek_past_loop_end_publish_normalized_cursor() {
    // Given
    let (_temp, path, descriptor) = fixture("wav-stream-normalized-loop", 16);
    let range = must(FrameLoopRange::new(
        FramePosition::new(4),
        FramePosition::new(8),
        16,
    ));
    let options = WavStreamOptions {
        start_frame: FramePosition::new(12),
        loop_range: Some(range),
    };
    let (mut control, mut callback) =
        must(must(validate_wav_file(&path, &descriptor)).open(options));

    // When
    must(control.play());
    let mut output = [0.0_f32; 2];
    callback.fill_output(&mut output);

    // Then
    assert_eq!(output, [4.0 / 32_768.0, -4.0 / 32_768.0]);
    assert_eq!(control.position(), FramePosition::new(5));
    must(control.seek(FramePosition::new(15)));
    assert_eq!(control.position(), FramePosition::new(4));
    must(control.shutdown());
}

#[test]
fn truncation_after_readiness_faults_worker_after_buffered_blocks_drain() {
    // Given
    let frames = STREAM_BLOCK_FRAMES * 12;
    let (_temp, path, descriptor) = fixture("wav-stream-post-ready-truncation", frames);
    let validated = must(validate_wav_file(&path, &descriptor));
    let (mut control, mut callback) = must(validated.open(WavStreamOptions::default()));
    let retained_frames = STREAM_BLOCK_FRAMES * 9;
    let retained_bytes = 44_u64 + must(u64::try_from(retained_frames * 4));
    must(must(std::fs::OpenOptions::new().write(true).open(&path)).set_len(retained_bytes));
    must(control.play());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut output = [0.0_f32; STREAM_BLOCK_FRAMES * 2];

    // When
    while control.state() != WavStreamState::Faulted && std::time::Instant::now() < deadline {
        callback.fill_output(&mut output);
        std::thread::yield_now();
    }

    // Then
    assert_eq!(control.state(), WavStreamState::Faulted);
    assert!(matches!(control.take_error(), Some(WavStreamError::Io(_))));
    assert!(control.shutdown().is_ok());
    assert_eq!(control.state(), WavStreamState::Shutdown);
}
