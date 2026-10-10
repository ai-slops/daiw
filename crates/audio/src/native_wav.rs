use cpal::traits::{DeviceTrait, StreamTrait};

use crate::native::{NativeRenderFormat, default_output_device_and_config};
use crate::{
    AudioError, FrameLoopRange, FramePosition, WavStreamControl, WavStreamError, WavStreamFile,
    WavStreamOptions, WavStreamState, WavStreamStats,
};

const WAV_STREAM_FORMAT: NativeRenderFormat = NativeRenderFormat {
    sample_rate: 48_000,
    channels: 2,
};

/// A CPAL output stream backed by a validated disk-streamed WAV file.
pub struct NativeWavPlayer {
    stream: Option<cpal::Stream>,
    control: WavStreamControl,
}

impl NativeWavPlayer {
    /// Opens the WAV worker and starts a continuously running native output stream.
    ///
    /// Playback remains paused until [`Self::play`] is called.
    ///
    /// # Errors
    /// Returns an error when the worker cannot start or the default output device cannot provide
    /// exact f32 stereo 48 kHz output.
    pub fn new(file: WavStreamFile, options: WavStreamOptions) -> Result<Self, AudioError> {
        let (device, supported) = default_output_device_and_config(WAV_STREAM_FORMAT)?;
        let (control, mut callback) = file.open(options)?;
        let device_errors = control.device_error_reporter();
        let stream = device
            .build_output_stream(
                &supported.config(),
                move |output: &mut [f32], _| callback.fill_output(output),
                move |_error| device_errors.record(),
                None,
            )
            .map_err(|error| AudioError::Native(error.to_string()))?;
        stream
            .play()
            .map_err(|error| AudioError::Native(error.to_string()))?;
        Ok(Self {
            stream: Some(stream),
            control,
        })
    }

    /// # Errors
    /// Returns an error when the stream has faulted or shut down.
    pub fn play(&self) -> Result<(), WavStreamError> {
        self.control.play()
    }

    /// # Errors
    /// Returns an error when the stream has faulted or shut down.
    pub fn pause(&self) -> Result<(), WavStreamError> {
        self.control.pause()
    }

    /// Stops playback and seeks to frame zero.
    ///
    /// # Errors
    /// Returns an error if the stream generation counter is exhausted.
    pub fn stop(&self) -> Result<(), WavStreamError> {
        self.control.stop()
    }

    /// Seeks to a stereo frame position.
    ///
    /// # Errors
    /// Returns an error when the frame is beyond the file or generations are exhausted.
    pub fn seek(&self, position: FramePosition) -> Result<(), WavStreamError> {
        self.control.seek(position)
    }

    /// Replaces the frame-domain loop range.
    ///
    /// # Errors
    /// Returns an error when the loop exceeds the file or generations are exhausted.
    pub fn set_loop(&self, loop_range: Option<FrameLoopRange>) -> Result<(), WavStreamError> {
        self.control.set_loop(loop_range)
    }

    #[must_use]
    pub fn state(&self) -> WavStreamState {
        self.control.state()
    }

    #[must_use]
    pub fn position(&self) -> FramePosition {
        self.control.position()
    }

    #[must_use]
    pub fn stats(&self) -> WavStreamStats {
        self.control.stats()
    }

    pub fn take_error(&self) -> Option<WavStreamError> {
        self.control.take_error()
    }

    /// Drops the device stream before joining the disk worker.
    ///
    /// # Errors
    /// Returns an error if the disk worker panicked.
    pub fn shutdown(&mut self) -> Result<(), WavStreamError> {
        drop(self.stream.take());
        self.control.shutdown()
    }

    #[must_use]
    pub const fn stream(&self) -> Option<&cpal::Stream> {
        self.stream.as_ref()
    }
}

impl Drop for NativeWavPlayer {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
