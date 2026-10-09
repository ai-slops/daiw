use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::pcm_format::lowercase_hex;
use crate::render::OUTPUT_BLOCK_FRAMES;
use crate::render_clock::RenderClock;
use crate::render_kernel::{BlockSink, RenderContext, render_blocks};
use crate::source::SourceBank;
use crate::validate::validate_snapshot;
use crate::wav::wav_header;
use crate::{AudioError, Snapshot};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WavFileDescriptor {
    pub format: &'static str,
    pub sample_rate: u32,
    pub channels: u16,
    pub frames: usize,
    pub byte_length: u64,
    pub sha256: String,
}

struct WavFileSink {
    path: PathBuf,
    file: Option<File>,
    hasher: Sha256,
    expected_frames: usize,
    written_frames: usize,
    complete: bool,
}

impl WavFileSink {
    fn create(path: &Path, frames: usize) -> Result<Self, AudioError> {
        if !path.is_absolute() {
            return Err(AudioError::InvalidRenderOutputPath);
        }
        let header = wav_header(crate::DEFAULT_SAMPLE_RATE, 2, frames)?;
        let file = OpenOptions::new().write(true).create_new(true).open(path)?;
        let hasher = Sha256::new();
        let mut sink = Self {
            path: path.to_owned(),
            file: Some(file),
            hasher,
            expected_frames: frames,
            written_frames: 0,
            complete: false,
        };
        let destination = sink.file.as_mut().ok_or(AudioError::InvalidPcmLength)?;
        destination.write_all(&header)?;
        sink.hasher.update(header);
        Ok(sink)
    }

    fn finish(mut self) -> Result<WavFileDescriptor, AudioError> {
        if self.written_frames != self.expected_frames {
            return Err(AudioError::InvalidFrameCount);
        }
        let mut file = self.file.take().ok_or(AudioError::InvalidPcmLength)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        let byte_length = u64::try_from(self.expected_frames)?
            .checked_mul(4)
            .and_then(|payload| payload.checked_add(44))
            .ok_or(AudioError::WavTooLarge)?;
        let digest = std::mem::take(&mut self.hasher).finalize();
        let sha256 = lowercase_hex(&digest);
        self.complete = true;
        Ok(WavFileDescriptor {
            format: "daiw-wav-pcm16-v1",
            sample_rate: crate::DEFAULT_SAMPLE_RATE,
            channels: 2,
            frames: self.expected_frames,
            byte_length,
            sha256,
        })
    }
}

impl Drop for WavFileSink {
    fn drop(&mut self) {
        if !self.complete {
            drop(self.file.take());
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

impl BlockSink for WavFileSink {
    fn write_block(&mut self, start_frame: usize, frames: &[[f32; 2]]) -> Result<(), AudioError> {
        if start_frame != self.written_frames
            || self.written_frames + frames.len() > self.expected_frames
        {
            return Err(AudioError::InvalidFrameCount);
        }
        let mut encoded = [0_u8; OUTPUT_BLOCK_FRAMES * 4];
        for (output, sample) in encoded.chunks_exact_mut(2).zip(frames.as_flattened()) {
            let pcm = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
            output.copy_from_slice(&pcm.to_le_bytes());
        }
        let bytes = &encoded[..frames.len() * 4];
        self.file
            .as_mut()
            .ok_or(AudioError::InvalidPcmLength)?
            .write_all(bytes)?;
        self.hasher.update(bytes);
        self.written_frames += frames.len();
        Ok(())
    }
}

pub(crate) fn render_to_file(
    snapshot: &Snapshot,
    sources: &mut impl SourceBank,
    output: &Path,
) -> Result<WavFileDescriptor, AudioError> {
    validate_snapshot(snapshot, crate::DEFAULT_SAMPLE_RATE)?;
    let clock = RenderClock::new(snapshot, crate::DEFAULT_SAMPLE_RATE)?;
    let mut sink = WavFileSink::create(output, clock.total_frames)?;
    render_blocks::<OUTPUT_BLOCK_FRAMES>(
        RenderContext {
            snapshot,
            clock: &clock,
        },
        sources,
        &mut sink,
    )?;
    sink.finish()
}
