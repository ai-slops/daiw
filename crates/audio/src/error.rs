use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("tempo must be finite and positive")]
    InvalidTempo,
    #[error("project length must be finite and non-negative")]
    InvalidLength,
    #[error("sample rate must be positive")]
    InvalidSampleRate,
    #[error("time cannot be represented as a finite frame index")]
    InvalidFrameCount,
    #[error("render exceeds the {limit_frames}-frame output limit")]
    RenderTooLarge { limit_frames: usize },
    #[error("audio buffer allocation failed: {0}")]
    Allocation(#[from] std::collections::TryReserveError),
    #[error("asset {asset_id} has invalid channel count {channels}")]
    InvalidAssetChannels { asset_id: String, channels: u16 },
    #[error("asset {asset_id} PCM length is not divisible by {channels} channels")]
    InvalidAssetLayout { asset_id: String, channels: u16 },
    #[error("asset {asset_id} is required by an audio region but was not supplied")]
    MissingAsset { asset_id: String },
    #[error("entity {entity_id} references unknown track {track_id}")]
    UnknownTrack { entity_id: String, track_id: String },
    #[error("entity {entity_id} has invalid {field}")]
    InvalidValue {
        entity_id: String,
        field: &'static str,
    },
    #[error("audio is too large to encode as PCM WAV")]
    WavTooLarge,
    #[error("invalid loop range: start {start} must be less than end {end}")]
    InvalidLoop { start: usize, end: usize },
    #[error("audio replacement queue is busy")]
    SwapBusy,
    #[error("JSON input is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("audio decode failed: {0}")]
    Decode(String),
    #[error("audio file has no decodable default track")]
    NoAudioTrack,
    #[error("--decode requires an absolute input path")]
    InvalidDecodePath,
    #[error("invalid command line: {0}")]
    InvalidArguments(String),
    #[error("decoded PCM resource limit exceeded: attempted {attempted} samples, limit {limit}")]
    ResourceLimit { limit: usize, attempted: usize },
    #[error("--decode-file requires absolute input and output paths")]
    InvalidDecodeFilePath,
    #[error("decoded packet exceeds the 8 MiB pre-conversion limit")]
    DecodedPacketTooLarge,
    #[error("decoded stream changes sample rate or channel count")]
    ChangingPcmFormat,
    #[error("decoded PCM contains a non-finite sample")]
    NonFiniteDecodedPcm,
    #[error("PCM sample rate {sample_rate} is outside 8000..=192000")]
    UnsupportedPcmSampleRate { sample_rate: u32 },
    #[error("PCM channel count {channels} is outside 1..=8")]
    UnsupportedPcmChannels { channels: u16 },
    #[error("PCM duration exceeds 180 seconds")]
    PcmDurationTooLong,
    #[error("PCM payload is invalid or exceeds 512 MiB")]
    PcmPayloadTooLarge,
    #[error("PCM header is invalid")]
    InvalidPcmHeader,
    #[error("PCM file length does not match its header")]
    InvalidPcmLength,
    #[error("numeric conversion failed: {0}")]
    IntegerConversion(#[from] std::num::TryFromIntError),
    #[error("binary field has an invalid length: {0}")]
    SliceConversion(#[from] std::array::TryFromSliceError),
    #[error("--source-root requires an absolute directory")]
    InvalidSourceRoot,
    #[error("--render-to-file requires an absolute output path")]
    InvalidRenderOutputPath,
    #[error("file-backed render supports at most {limit} sources")]
    TooManySources { limit: usize },
    #[error("source files exceed the 1 GiB aggregate limit")]
    SourceFilesTooLarge,
    #[error("source path escapes its root")]
    SourceEscapesRoot,
    #[error("source {asset_id} was supplied but is not referenced")]
    ExtraSource { asset_id: String },
    #[error("cacheId and sha256 must be 64 lowercase hexadecimal characters")]
    InvalidSha256,
    #[error("PCM metadata does not match the source descriptor")]
    PcmMetadataMismatch,
    #[error("PCM payload SHA-256 does not match its header")]
    PcmPayloadHashMismatch,
    #[error("PCM whole-file SHA-256 does not match the source descriptor")]
    PcmFileHashMismatch,
    #[error(transparent)]
    WavStream(#[from] crate::WavStreamError),
    #[cfg(feature = "native")]
    #[error("no default audio output device is available")]
    NoOutputDevice,
    #[cfg(feature = "native")]
    #[error("native audio backend failed: {0}")]
    Native(String),
}
