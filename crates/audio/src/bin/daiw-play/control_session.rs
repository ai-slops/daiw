use daiw_audio::{
    FrameLoopRange, FramePosition, NativeWavPlayer, WavStreamCallback, WavStreamControl,
    WavStreamError, WavStreamFile, WavStreamOptions, WavStreamState, WavStreamStats,
};

pub trait ControlSession {
    fn play(&self) -> Result<(), WavStreamError>;
    fn pause(&self) -> Result<(), WavStreamError>;
    fn stop(&self) -> Result<(), WavStreamError>;
    fn seek(&self, position: FramePosition) -> Result<(), WavStreamError>;
    fn set_loop(&self, loop_range: Option<FrameLoopRange>) -> Result<(), WavStreamError>;
    fn state(&self) -> WavStreamState;
    fn position(&self) -> FramePosition;
    fn stats(&self) -> WavStreamStats;
    fn take_error(&self) -> Option<WavStreamError>;
    fn advance(&mut self, frames: u16) -> Result<(), SessionError>;
    fn shutdown(&mut self) -> Result<(), WavStreamError>;
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("advance is available only with --headless-callback-driver")]
    AdvanceUnavailable,
    #[error("advance frames must be in 1..=4096")]
    InvalidAdvance,
    #[error("headless callback data was not ready within one second")]
    HeadlessTimeout,
    #[error(transparent)]
    Stream(#[from] WavStreamError),
}

pub struct NativeSession(pub NativeWavPlayer);

impl ControlSession for NativeSession {
    fn play(&self) -> Result<(), WavStreamError> {
        self.0.play()
    }
    fn pause(&self) -> Result<(), WavStreamError> {
        self.0.pause()
    }
    fn stop(&self) -> Result<(), WavStreamError> {
        self.0.stop()
    }
    fn seek(&self, position: FramePosition) -> Result<(), WavStreamError> {
        self.0.seek(position)
    }
    fn set_loop(&self, range: Option<FrameLoopRange>) -> Result<(), WavStreamError> {
        self.0.set_loop(range)
    }
    fn state(&self) -> WavStreamState {
        self.0.state()
    }
    fn position(&self) -> FramePosition {
        self.0.position()
    }
    fn stats(&self) -> WavStreamStats {
        self.0.stats()
    }
    fn take_error(&self) -> Option<WavStreamError> {
        self.0.take_error()
    }
    fn advance(&mut self, _frames: u16) -> Result<(), SessionError> {
        Err(SessionError::AdvanceUnavailable)
    }
    fn shutdown(&mut self) -> Result<(), WavStreamError> {
        self.0.shutdown()
    }
}

pub struct HeadlessSession {
    control: WavStreamControl,
    callback: WavStreamCallback,
}

impl HeadlessSession {
    pub fn new(file: WavStreamFile) -> Result<Self, WavStreamError> {
        let (control, callback) = file.open(WavStreamOptions::default())?;
        Ok(Self { control, callback })
    }
}

impl ControlSession for HeadlessSession {
    fn play(&self) -> Result<(), WavStreamError> {
        self.control.play()
    }
    fn pause(&self) -> Result<(), WavStreamError> {
        self.control.pause()
    }
    fn stop(&self) -> Result<(), WavStreamError> {
        self.control.stop()
    }
    fn seek(&self, position: FramePosition) -> Result<(), WavStreamError> {
        self.control.seek(position)
    }
    fn set_loop(&self, range: Option<FrameLoopRange>) -> Result<(), WavStreamError> {
        self.control.set_loop(range)
    }
    fn state(&self) -> WavStreamState {
        self.control.state()
    }
    fn position(&self) -> FramePosition {
        self.control.position()
    }
    fn stats(&self) -> WavStreamStats {
        self.control.stats()
    }
    fn take_error(&self) -> Option<WavStreamError> {
        self.control.take_error()
    }
    fn advance(&mut self, frames: u16) -> Result<(), SessionError> {
        if frames == 0 || usize::from(frames) > daiw_audio::STREAM_BLOCK_FRAMES {
            return Err(SessionError::InvalidAdvance);
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        while !self.callback.prepare_output() {
            if std::time::Instant::now() >= deadline {
                return Err(SessionError::HeadlessTimeout);
            }
            std::thread::park_timeout(std::time::Duration::from_millis(1));
        }
        let mut output = vec![0.0_f32; usize::from(frames) * 2];
        self.callback.fill_output(&mut output);
        Ok(())
    }
    fn shutdown(&mut self) -> Result<(), WavStreamError> {
        self.control.shutdown()
    }
}
