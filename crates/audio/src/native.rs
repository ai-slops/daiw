use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::{AudioError, AudioSwap, AudioSwapControl, LoopRange, RenderedAudio, Transport};

#[derive(Debug, Default)]
struct StreamErrors(AtomicU64);

impl StreamErrors {
    fn record(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }

    fn count(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

pub struct NativePlayer {
    stream: cpal::Stream,
    transport: Arc<Transport>,
    swaps: AudioSwapControl,
    sample_rate: u32,
    channels: u16,
    errors: Arc<StreamErrors>,
}

impl NativePlayer {
    pub fn new(audio: RenderedAudio) -> Result<Self, AudioError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(AudioError::NoOutputDevice)?;
        let supported = device
            .default_output_config()
            .map_err(|error| AudioError::Native(error.to_string()))?;
        if supported.sample_format() != cpal::SampleFormat::F32 {
            return Err(AudioError::Native(format!(
                "default output format {:?} is unsupported; f32 required",
                supported.sample_format()
            )));
        }
        if supported.sample_rate().0 != audio.sample_rate || supported.channels() != audio.channels
        {
            return Err(AudioError::Native(format!(
                "device format is {} Hz/{} channels, render is {} Hz/{} channels",
                supported.sample_rate().0,
                supported.channels(),
                audio.sample_rate,
                audio.channels
            )));
        }
        let samples: Arc<[f32]> = audio.samples.into();
        let transport = Arc::new(Transport::new(samples.len()));
        let (swaps, mut callback) = AudioSwap::channel(Arc::clone(&transport), samples);
        let errors = Arc::new(StreamErrors::default());
        let callback_errors = Arc::clone(&errors);
        let stream = device
            .build_output_stream(
                &supported.config(),
                move |output: &mut [f32], _| {
                    callback.fill(output);
                },
                move |_error| callback_errors.record(),
                None,
            )
            .map_err(|error| AudioError::Native(error.to_string()))?;
        stream
            .play()
            .map_err(|error| AudioError::Native(error.to_string()))?;
        Ok(Self {
            stream,
            transport,
            swaps,
            sample_rate: supported.sample_rate().0,
            channels: supported.channels(),
            errors,
        })
    }

    pub fn play(&self) {
        self.transport.play();
    }

    pub fn pause(&self) {
        self.transport.pause();
    }

    pub fn seek(&self, interleaved_sample_offset: usize) {
        self.transport.seek(interleaved_sample_offset);
    }

    pub fn set_loop(&self, range: Option<LoopRange>) {
        self.transport.set_loop(range);
    }

    #[must_use]
    pub fn position(&self) -> usize {
        self.transport.position()
    }

    #[must_use]
    pub fn sample_count(&self) -> usize {
        self.transport.length()
    }

    pub fn replace_audio(&mut self, audio: RenderedAudio) -> Result<(), AudioError> {
        if audio.sample_rate != self.sample_rate || audio.channels != self.channels {
            return Err(AudioError::Native(format!(
                "replacement is {} Hz/{} channels, stream is {} Hz/{} channels",
                audio.sample_rate, audio.channels, self.sample_rate, self.channels
            )));
        }
        self.swaps.enqueue(audio.samples.into())
    }

    pub fn reclaim_replaced_audio(&mut self) -> usize {
        self.swaps.reclaim()
    }

    #[must_use]
    pub fn error_count(&self) -> u64 {
        self.errors.count()
    }

    pub const fn stream(&self) -> &cpal::Stream {
        &self.stream
    }
}

#[cfg(test)]
mod tests {
    use super::StreamErrors;

    #[test]
    fn stream_errors_are_counted_without_formatting_or_logging() {
        // Given
        let errors = StreamErrors::default();

        // When
        errors.record();
        errors.record();

        // Then
        assert_eq!(errors.count(), 2);
    }
}
