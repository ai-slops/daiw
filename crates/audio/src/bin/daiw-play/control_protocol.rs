use daiw_audio::{FrameLoopRange, FramePosition, WavStreamState, WavStreamStats};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Request {
    #[serde(rename = "play")]
    Play { v: u8, id: u64 },
    #[serde(rename = "pause")]
    Pause { v: u8, id: u64 },
    #[serde(rename = "stop")]
    Stop { v: u8, id: u64 },
    #[serde(rename = "seek")]
    Seek { v: u8, id: u64, frame: u32 },
    #[serde(rename = "setLoop")]
    SetLoop {
        v: u8,
        id: u64,
        #[serde(rename = "loop")]
        loop_range: Option<LoopInput>,
    },
    #[serde(rename = "state")]
    State { v: u8, id: u64 },
    #[serde(rename = "shutdown")]
    Shutdown { v: u8, id: u64 },
    #[serde(rename = "advance")]
    Advance { v: u8, id: u64, frames: u16 },
}

impl Request {
    pub const fn envelope(&self) -> (u8, u64) {
        match *self {
            Self::Play { v, id }
            | Self::Pause { v, id }
            | Self::Stop { v, id }
            | Self::Seek { v, id, .. }
            | Self::SetLoop { v, id, .. }
            | Self::State { v, id }
            | Self::Shutdown { v, id }
            | Self::Advance { v, id, .. } => (v, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LoopInput {
    pub start: u32,
    pub end: u32,
}

impl LoopInput {
    pub const fn into_range(
        self,
        frames: u32,
    ) -> Result<FrameLoopRange, daiw_audio::WavStreamError> {
        FrameLoopRange::new(
            FramePosition::new(self.start),
            FramePosition::new(self.end),
            frames,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Stopped,
    Paused,
    Playing,
    Faulted,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateBody {
    pub status: Status,
    pub position_frame: u32,
    pub frames: u32,
    #[serde(rename = "loop")]
    pub loop_range: Option<LoopInput>,
    pub stats: StatsBody,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsBody {
    pub underrun_callbacks: u64,
    pub stale_blocks: u64,
    pub device_errors: u64,
}

impl From<WavStreamStats> for StatsBody {
    fn from(stats: WavStreamStats) -> Self {
        Self {
            underrun_callbacks: stats.underrun_callbacks,
            stale_blocks: stats.stale_blocks,
            device_errors: stats.device_errors,
        }
    }
}

pub const fn observed_status(state: WavStreamState, logical: Status) -> Status {
    match state {
        WavStreamState::Paused => logical,
        WavStreamState::Playing => Status::Playing,
        WavStreamState::Ended => Status::Stopped,
        WavStreamState::Faulted | WavStreamState::Shutdown => Status::Faulted,
    }
}

#[derive(Debug, Serialize)]
pub struct ErrorBody<'a> {
    pub code: &'a str,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum Output<'a> {
    #[serde(rename = "ready")]
    Ready { v: u8, frames: u32 },
    #[serde(rename = "response")]
    Response {
        v: u8,
        id: u64,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        state: Option<StateBody>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<ErrorBody<'a>>,
    },
    #[serde(rename = "state")]
    State {
        v: u8,
        sequence: u64,
        state: StateBody,
    },
    #[serde(rename = "fatal")]
    Fatal { v: u8, error: ErrorBody<'a> },
}
