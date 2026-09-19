use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("tempo must be finite and positive")]
    InvalidTempo,
    #[error("project length must be finite and non-negative")]
    InvalidLength,
    #[error("sample rate must be positive")]
    InvalidSampleRate,
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
    #[cfg(feature = "native")]
    #[error("no default audio output device is available")]
    NoOutputDevice,
    #[cfg(feature = "native")]
    #[error("native audio backend failed: {0}")]
    Native(String),
}
