mod decode;
mod decode_stream;
mod dsp_executor;
mod dsp_sources;
mod error;
mod file_bank;
mod file_validation;
mod loop_state;
mod model;
#[cfg(feature = "native")]
mod native;
#[cfg(feature = "native")]
mod native_device;
#[cfg(feature = "native")]
mod native_wav;
mod pcm_format;
mod render;
mod render_clock;
mod render_kernel;
mod render_synthesis;
mod signal_graph;
mod source;
mod transport;
#[cfg(test)]
mod transport_concurrency;
mod validate;
mod wav;
mod wav_file;
mod wav_stream;

pub use decode::{MAX_DECODED_SAMPLES, decode_file};
pub use decode_stream::decode_to_pcm;
pub use dsp_executor::{DspExecutor, DspProcessError, DspProcessReport, DspProcessState};
pub use dsp_sources::ResidentDspSources;
pub use error::AudioError;
pub use file_bank::{
    FileRenderInput, FileSourceDescriptor, render_file_backed, render_file_backed_to_wav,
};
pub use model::{
    AudioRegion, DecodedAsset, EffectiveNote, FlatRenderInput, Instrument, Origin, PcmAsset,
    RenderEnvelope, RenderInput, RenderRequest, RenderedAudio, Snapshot, Track,
};
#[cfg(feature = "native")]
pub use native::NativePlayer;
#[cfg(feature = "native")]
pub use native_device::{NativeDeviceSelector, NativeOutputDevice, list_native_output_devices};
#[cfg(feature = "native")]
pub use native_wav::NativeWavPlayer;
pub use pcm_format::{PCM_HEADER_LEN, PCM_PEAK_COUNT, PcmDescriptor, PcmHeader};
pub use render::{DEFAULT_SAMPLE_RATE, render, render_with_sample_rate};
pub use render_clock::MAX_RENDER_FRAMES;
pub use signal_graph::{CompiledDspProgram, MAX_DSP_PREPARED_EVENTS};
pub use source::{SourceMetadata, SourceMetadataProvider};
pub use transport::{
    AudioSwap, AudioSwapCallback, AudioSwapControl, CallbackBuffer, LoopRange, Transport,
};
pub use wav::wav;
pub use wav_file::WavFileDescriptor;
pub use wav_stream::{
    FrameLoopRange, FramePosition, MAX_WAV_STREAM_FRAMES, STREAM_BLOCK_FRAMES, STREAM_RING_BLOCKS,
    Sha256Digest, StreamGeneration, WavStreamCallback, WavStreamControl, WavStreamError,
    WavStreamFile, WavStreamOptions, WavStreamState, WavStreamStats, streaming_pair,
    validate_wav_file,
};
