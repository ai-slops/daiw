use std::io::Write;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use daiw_audio::{AudioError, FramePosition, WavStreamError, WavStreamState};

use super::control_io::{Input, input_receiver, write_output};
use super::control_protocol::{ErrorBody, LoopInput, Output, Request, StateBody, Status};
use super::control_session::{ControlSession, SessionError};

const STATE_INTERVAL: Duration = Duration::from_millis(50);

pub fn run(mut session: Box<dyn ControlSession>, frames: u32) -> Result<(), AudioError> {
    let receiver = input_receiver()?;
    let mut output = std::io::stdout().lock();
    write_output(&mut output, &Output::Ready { v: 1, frames })?;
    let result = {
        let mut worker = Worker {
            session: session.as_mut(),
            frames,
            loop_range: None,
            logical: Status::Stopped,
            last_status: Status::Stopped,
            sequence: 0,
            output: &mut output,
        };
        worker.event_loop(&receiver)
    };
    let shutdown = session.shutdown();
    result?;
    shutdown?;
    Ok(())
}

struct Worker<'a> {
    session: &'a mut dyn ControlSession,
    frames: u32,
    loop_range: Option<LoopInput>,
    logical: Status,
    last_status: Status,
    sequence: u64,
    output: &'a mut dyn Write,
}

impl Worker<'_> {
    fn event_loop(&mut self, receiver: &Receiver<Input>) -> Result<(), AudioError> {
        loop {
            match receiver.recv_timeout(STATE_INTERVAL) {
                Ok(Input::Line(line)) => {
                    let request = match serde_json::from_slice::<Request>(&line) {
                        Ok(request) => request,
                        Err(error) => return self.fatal("InvalidRequest", error.to_string()),
                    };
                    if request.envelope().0 != 1 {
                        return self
                            .fatal("UnsupportedVersion", "protocol version must be 1".into());
                    }
                    if self.handle(request)? {
                        return Ok(());
                    }
                    if self.faulted()? {
                        return Err(protocol_failure());
                    }
                }
                Ok(Input::TooLong) => {
                    return self.fatal("LineTooLong", "input line exceeds 4096 bytes".into());
                }
                Ok(Input::Io(error)) => return self.fatal("InputIo", error.to_string()),
                Ok(Input::Eof) | Err(RecvTimeoutError::Disconnected) => return Ok(()),
                Err(RecvTimeoutError::Timeout) => {
                    let status = self.snapshot().status;
                    if status == Status::Playing || status != self.last_status {
                        self.publish_state()?;
                    }
                    if self.faulted()? {
                        return Err(protocol_failure());
                    }
                }
            }
        }
    }

    fn handle(&mut self, request: Request) -> Result<bool, AudioError> {
        self.sync_logical(None);
        let (_, id) = request.envelope();
        let result = match request {
            Request::Play { .. } => self
                .session
                .play()
                .map(|()| self.sync_logical(Some(Status::Paused))),
            Request::Pause { .. } => self
                .session
                .pause()
                .map(|()| self.sync_logical(Some(Status::Paused))),
            Request::Stop { .. } => self.session.stop().map(|()| self.logical = Status::Stopped),
            Request::Seek { frame, .. } => self
                .session
                .seek(FramePosition::new(frame))
                .map(|()| self.sync_logical(Some(Status::Paused))),
            Request::SetLoop { loop_range, .. } => {
                let range = loop_range
                    .map(|value| value.into_range(self.frames))
                    .transpose();
                match range {
                    Ok(range) => self
                        .session
                        .set_loop(range)
                        .map(|()| self.loop_range = loop_range),
                    Err(error) => Err(error),
                }
            }
            Request::State { .. } => Ok(()),
            Request::Shutdown { .. } => {
                let state = self.snapshot();
                write_output(
                    self.output,
                    &Output::Response {
                        v: 1,
                        id,
                        ok: true,
                        state: Some(state),
                        error: None,
                    },
                )?;
                return Ok(true);
            }
            Request::Advance { frames, .. } => match self.session.advance(frames) {
                Ok(()) => {
                    self.sync_logical(None);
                    Ok(())
                }
                Err(error) => return self.command_error(id, &error),
            },
        };
        match result {
            Ok(()) => {
                let state = self.snapshot();
                write_output(
                    self.output,
                    &Output::Response {
                        v: 1,
                        id,
                        ok: true,
                        state: Some(state),
                        error: None,
                    },
                )?;
                self.publish_state()?;
            }
            Err(error) => return self.command_error(id, &SessionError::Stream(error)),
        }
        Ok(false)
    }

    fn command_error(&mut self, id: u64, error: &SessionError) -> Result<bool, AudioError> {
        let code = match error {
            SessionError::AdvanceUnavailable => "UnsupportedCommand",
            SessionError::InvalidAdvance => "InvalidAdvance",
            SessionError::HeadlessTimeout => "AudioError",
            SessionError::Stream(WavStreamError::InvalidStartFrame) => "InvalidFrame",
            SessionError::Stream(WavStreamError::InvalidLoopRange) => "InvalidLoop",
            SessionError::Stream(_) => "AudioError",
        };
        write_output(
            self.output,
            &Output::Response {
                v: 1,
                id,
                ok: false,
                state: None,
                error: Some(ErrorBody {
                    code,
                    message: error.to_string(),
                }),
            },
        )?;
        Ok(false)
    }

    fn snapshot(&self) -> StateBody {
        StateBody {
            status: super::control_protocol::observed_status(self.session.state(), self.logical),
            position_frame: self.session.position().get(),
            frames: self.frames,
            loop_range: self.loop_range,
            stats: self.session.stats().into(),
        }
    }

    fn sync_logical(&mut self, paused: Option<Status>) {
        match self.session.state() {
            WavStreamState::Paused => {
                if let Some(status) = paused {
                    self.logical = status;
                }
            }
            WavStreamState::Playing => self.logical = Status::Playing,
            WavStreamState::Ended => self.logical = Status::Stopped,
            WavStreamState::Faulted | WavStreamState::Shutdown => {
                self.logical = Status::Faulted;
            }
        }
    }

    fn publish_state(&mut self) -> Result<(), AudioError> {
        self.sequence = self.sequence.saturating_add(1);
        let state = self.snapshot();
        self.last_status = state.status;
        write_output(
            self.output,
            &Output::State {
                v: 1,
                sequence: self.sequence,
                state,
            },
        )
    }

    fn faulted(&mut self) -> Result<bool, AudioError> {
        if self.session.state() != daiw_audio::WavStreamState::Faulted {
            return Ok(false);
        }
        self.logical = Status::Faulted;
        self.publish_state()?;
        let message = self.session.take_error().map_or_else(
            || "audio stream faulted".to_owned(),
            |error| error.to_string(),
        );
        write_output(
            self.output,
            &Output::Fatal {
                v: 1,
                error: ErrorBody {
                    code: "AudioError",
                    message,
                },
            },
        )?;
        Ok(true)
    }

    fn fatal(&mut self, code: &'static str, message: String) -> Result<(), AudioError> {
        write_output(
            self.output,
            &Output::Fatal {
                v: 1,
                error: ErrorBody { code, message },
            },
        )?;
        Err(protocol_failure())
    }
}

fn protocol_failure() -> AudioError {
    AudioError::ControlProtocol
}
