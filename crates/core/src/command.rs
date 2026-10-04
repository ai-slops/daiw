use crate::ids::{DefinitionId, InstanceId, ModifierId, RegionId, Revision, TrackId};
use crate::music::{Instrument, Note, Operation};
use crate::project::{Asset, AudioRegion};
use crate::time::Tick;
use daiw_timing::{MeterMap, TempoMap};
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
    SetTempoMap {
        tempo_map: TempoMap,
    },
    SetMeter {
        meter: [u8; 2],
    },
    SetMeterMap {
        meter_map: MeterMap,
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
    TransformSelectedNotes {
        selection: crate::note_selection::SelectionHandle,
        transform: crate::selected_note_edit::NoteTransform,
    },
    ApplyModifier {
        instance_id: InstanceId,
        modifier_id: ModifierId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ordering: Option<crate::ordering_graph::ModifierOrdering>,
    },
    SetModifierOrdering {
        instance_id: InstanceId,
        ordering: crate::ordering_graph::ModifierOrdering,
    },
    RebaseModifier {
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
