mod decode;
mod error;
mod loop_state;
mod model;
#[cfg(feature = "native")]
mod native;
mod render;
mod render_clock;
mod transport;
#[cfg(test)]
mod transport_concurrency;
mod validate;
mod wav;

pub use decode::{MAX_DECODED_SAMPLES, decode_file};
pub use error::AudioError;
pub use model::{
    AudioRegion, DecodedAsset, EffectiveNote, FlatRenderInput, Instrument, Origin, PcmAsset,
    RenderEnvelope, RenderInput, RenderRequest, RenderedAudio, Snapshot, Track,
};
#[cfg(feature = "native")]
pub use native::NativePlayer;
pub use render::{DEFAULT_SAMPLE_RATE, render, render_with_sample_rate};
pub use render_clock::MAX_RENDER_FRAMES;
pub use transport::{
    AudioSwap, AudioSwapCallback, AudioSwapControl, CallbackBuffer, LoopRange, Transport,
};
pub use wav::wav;
