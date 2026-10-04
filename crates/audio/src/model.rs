use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub revision: String,
    pub tempo: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tempo_map: Option<daiw_timing::TempoMap>,
    pub length: f64,
    pub tracks: Vec<Track>,
    pub notes: Vec<EffectiveNote>,
    #[serde(default)]
    pub regions: Vec<AudioRegion>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: String,
    pub name: String,
    pub role: String,
    pub color: String,
    pub instrument: Instrument,
    pub gain: f64,
    pub pan: f64,
    pub mute: bool,
    pub solo: bool,
    pub cutoff: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Instrument {
    Synth,
    Bass,
    Kick,
    Snare,
    Hat,
    Pad,
    Sample,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveNote {
    pub id: String,
    pub start: f64,
    pub duration: f64,
    pub pitch: f64,
    pub velocity: f64,
    pub instance_id: String,
    pub track_id: String,
    pub origin: Origin,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Origin {
    pub definition_id: String,
    pub note_id: String,
    pub modifier_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioRegion {
    pub id: String,
    pub track_id: String,
    pub asset_id: String,
    pub start: f64,
    pub duration: f64,
    pub gain: f64,
    pub source_offset: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PcmAsset {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderEnvelope {
    pub snapshot: Snapshot,
    #[serde(default)]
    pub assets: BTreeMap<String, PcmAsset>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlatRenderInput {
    #[serde(flatten)]
    pub snapshot: Snapshot,
    pub assets: BTreeMap<String, PcmAsset>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RenderInput {
    Envelope(RenderEnvelope),
    Flat(FlatRenderInput),
    Snapshot(Snapshot),
}

#[derive(Clone, Debug)]
pub struct RenderRequest {
    pub snapshot: Snapshot,
    pub assets: BTreeMap<String, PcmAsset>,
}

impl RenderRequest {
    #[must_use]
    pub const fn new(snapshot: Snapshot) -> Self {
        Self {
            snapshot,
            assets: BTreeMap::new(),
        }
    }
}

impl From<RenderInput> for RenderRequest {
    fn from(input: RenderInput) -> Self {
        match input {
            RenderInput::Envelope(envelope) => Self {
                snapshot: envelope.snapshot,
                assets: envelope.assets,
            },
            RenderInput::Flat(flat) => Self {
                snapshot: flat.snapshot,
                assets: flat.assets,
            },
            RenderInput::Snapshot(snapshot) => Self::new(snapshot),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderedAudio {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecodedAsset {
    pub frames: usize,
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}
