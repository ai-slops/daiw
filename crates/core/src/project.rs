use crate::ids::{AssetId, HistoryId, ProjectId, RegionId, Revision, TrackId};
use crate::music::{Definition, EffectiveNote, Instance, Modifier, Track};
use crate::proposal_model::Proposal;
use crate::time::Tick;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: HistoryId,
    pub message: String,
    pub author: String,
    pub revision: Revision,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: AssetId,
    pub name: String,
    pub hash: String,
    pub frames: u64,
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioRegion {
    pub id: RegionId,
    pub track_id: TrackId,
    pub asset_id: AssetId,
    pub start: Tick,
    pub duration: Tick,
    pub gain: f64,
    pub source_offset: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub revision: Revision,
    pub tempo: f64,
    pub meter: [u8; 2],
    pub definitions: Vec<Definition>,
    pub instances: Vec<Instance>,
    pub tracks: Vec<Track>,
    pub modifiers: Vec<Modifier>,
    pub proposals: Vec<Proposal>,
    pub history: Vec<HistoryEntry>,
    #[serde(default)]
    pub regions: Vec<AudioRegion>,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub revision: Revision,
    pub tempo: f64,
    pub length: Tick,
    pub tracks: Vec<Track>,
    pub notes: Vec<EffectiveNote>,
    pub regions: Vec<AudioRegion>,
}
