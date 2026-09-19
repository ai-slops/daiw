use crate::ids::{DefinitionId, InstanceId, ModifierId, RegionId, Revision, TrackId};
use crate::music::{Instrument, Note, Operation};
use crate::project::{Asset, AudioRegion};
use crate::time::Tick;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EditScope {
    Source,
    Instance,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "type"
)]
pub enum Command {
    SetTempo {
        tempo: f64,
    },
    RenameProject {
        name: String,
    },
    AddTrack {
        name: String,
        instrument: Instrument,
        color: String,
        role: String,
    },
    SetTrack {
        track_id: TrackId,
        gain: Option<f64>,
        pan: Option<f64>,
        mute: Option<bool>,
        solo: Option<bool>,
        cutoff: Option<f64>,
        name: Option<String>,
    },
    AddDefinition {
        name: String,
        length: Tick,
        notes: Vec<Note>,
    },
    Instantiate {
        definition_id: DefinitionId,
        track_id: TrackId,
        start: Tick,
        name: Option<String>,
    },
    MoveInstance {
        instance_id: InstanceId,
        start: Tick,
        track_id: Option<TrackId>,
    },
    DuplicateInstance {
        instance_id: InstanceId,
    },
    DeleteInstance {
        instance_id: InstanceId,
    },
    EditNotes {
        instance_id: InstanceId,
        scope: EditScope,
        notes: Vec<Note>,
    },
    ApplyModifier {
        instance_id: InstanceId,
        modifier_id: ModifierId,
    },
    RemoveModifier {
        instance_id: InstanceId,
        modifier_id: ModifierId,
    },
    CreateModifier {
        name: String,
        operations: Vec<Operation>,
    },
    AddAudioRegion {
        asset: Asset,
        region: AudioRegion,
    },
    RemoveAudioRegion {
        region_id: RegionId,
    },
}

#[derive(Clone, Debug)]
pub struct ChangeRequest {
    pub expected_revision: Revision,
    pub command: Command,
}
