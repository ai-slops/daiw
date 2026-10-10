use std::fmt;

use crate::WavFileDescriptor;

pub const STREAM_BLOCK_FRAMES: usize = 4096;
/// Eight ring slots plus one worker-local and one callback-local decoded block bound ownership to
/// ten blocks, approximately 328 KiB in total rather than the ring capacity alone.
pub const STREAM_RING_BLOCKS: usize = 8;
pub const MAX_WAV_STREAM_FRAMES: u32 = 8_640_000;

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct FramePosition(u32);

impl FramePosition {
    #[must_use]
    pub const fn new(frame: u32) -> Self {
        Self(frame)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameLoopRange {
    start: FramePosition,
    end: FramePosition,
}

impl FrameLoopRange {
    pub const fn new(
        start: FramePosition,
        end: FramePosition,
        frames: u32,
    ) -> Result<Self, WavStreamError> {
        if start.0 >= end.0 || end.0 > frames {
            return Err(WavStreamError::InvalidLoopRange);
        }
        Ok(Self { start, end })
    }

    #[must_use]
    pub const fn start(self) -> FramePosition {
        self.start
    }

    #[must_use]
    pub const fn end(self) -> FramePosition {
        self.end
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamGeneration(pub(crate) u32);

impl StreamGeneration {
    pub(crate) const INITIAL: Self = Self(0);

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    pub(crate) const fn next(self) -> Result<Self, WavStreamError> {
        match self.0.checked_add(1) {
            Some(value) => Ok(Self(value)),
            None => Err(WavStreamError::GenerationExhausted),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WavStreamOptions {
    pub start_frame: FramePosition,
    pub loop_range: Option<FrameLoopRange>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WavStreamState {
    Paused,
    Playing,
    Ended,
    Faulted,
    Shutdown,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WavStreamStats {
    pub underrun_callbacks: u64,
    pub stale_blocks: u64,
    pub device_errors: u64,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Sha256Digest(pub(crate) [u8; 32]);

impl Sha256Digest {
    pub fn parse(value: &str) -> Result<Self, WavStreamError> {
        if value.len() != 64 {
            return Err(WavStreamError::InvalidSha256);
        }
        let mut bytes = [0_u8; 32];
        for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
            let high = decode_hex(pair[0]).ok_or(WavStreamError::InvalidSha256)?;
            let low = decode_hex(pair[1]).ok_or(WavStreamError::InvalidSha256)?;
            bytes[index] = (high << 4) | low;
        }
        Ok(Self(bytes))
    }
}

impl fmt::Debug for Sha256Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Sha256Digest(..)")
    }
}

const fn decode_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WavStreamError {
    #[error("WAV stream path must be absolute")]
    RelativePath,
    #[error("SHA-256 must contain exactly 64 lowercase hexadecimal characters")]
    InvalidSha256,
    #[error("WAV stream descriptor does not match the canonical PCM16 stereo format")]
    DescriptorMismatch,
    #[error("WAV stream header is not canonical 44-byte PCM16 stereo 48000 Hz")]
    InvalidHeader,
    #[error("WAV stream exceeds the frame limit")]
    TooManyFrames,
    #[error("WAV stream start frame is outside the file")]
    InvalidStartFrame,
    #[error("WAV stream loop range is invalid")]
    InvalidLoopRange,
    #[error("WAV stream generation counter is exhausted")]
    GenerationExhausted,
    #[error("WAV stream is faulted")]
    StreamFaulted,
    #[error("WAV stream is shut down")]
    StreamShutdown,
    #[error("WAV stream worker panicked")]
    WorkerPanicked,
    #[error("WAV stream numeric conversion failed: {0}")]
    IntegerConversion(#[from] std::num::TryFromIntError),
    #[error("WAV stream I/O failed: {0}")]
    Io(#[from] std::io::Error),
}

pub(crate) fn descriptor_frames(descriptor: &WavFileDescriptor) -> Result<u32, WavStreamError> {
    u32::try_from(descriptor.frames).map_err(|_| WavStreamError::TooManyFrames)
}

pub(crate) const fn normalize_position(
    position: FramePosition,
    loop_range: Option<FrameLoopRange>,
) -> FramePosition {
    match loop_range {
        Some(range) if position.get() >= range.end().get() => range.start(),
        Some(_) | None => position,
    }
}
