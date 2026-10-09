mod decode;
mod decode_stream;
mod error;
mod file_bank;
mod file_validation;
mod loop_state;
mod model;
#[cfg(feature = "native")]
mod native;
mod pcm_format;
mod render;
mod render_clock;
mod render_kernel;
mod render_plan;
mod source;
mod transport;
#[cfg(test)]
mod transport_concurrency;
mod validate;
mod wav;
mod wav_file;

pub use decode::{MAX_DECODED_SAMPLES, decode_file};
pub use decode_stream::decode_to_pcm;
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
pub use pcm_format::{PCM_HEADER_LEN, PCM_PEAK_COUNT, PcmDescriptor, PcmHeader};
pub use render::{DEFAULT_SAMPLE_RATE, render, render_with_sample_rate};
pub use render_clock::MAX_RENDER_FRAMES;
pub use transport::{
    AudioSwap, AudioSwapCallback, AudioSwapControl, CallbackBuffer, LoopRange, Transport,
};
pub use wav::wav;
pub use wav_file::WavFileDescriptor;
