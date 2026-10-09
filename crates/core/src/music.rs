use crate::ids::{ChildPlacementId, DefinitionId, InstanceId, ModifierId, NoteId, TrackId};
use crate::time::Tick;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: NoteId,
    pub start: Tick,
    pub duration: Tick,
    pub pitch: f64,
    pub velocity: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Definition {
    pub id: DefinitionId,
    pub name: String,
    pub length: Tick,
    pub notes: Vec<Note>,
    pub revision: u64,
    #[serde(default)]
    pub slots: BTreeMap<String, NoteSlot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ChildPlacement>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildPlacement {
    pub id: ChildPlacementId,
    pub definition_id: DefinitionId,
    pub start: Tick,
    pub length: Tick,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSlot {
    pub start: Tick,
    pub end: Tick,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Instrument {
    Synth,
    Bass,
    Kick,
    Snare,
    Hat,
    Pad,
    Sample,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: TrackId,
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

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Parameter {
    Cutoff,
    Pan,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Operation {
    Transpose { semitones: f64 },
    Velocity { factor: f64 },
    Gain { db: f64 },
    AddNotes { notes: Vec<Note> },
    ReplaceSlot { slot: String, notes: Vec<Note> },
    PatchNotes(crate::note_patch::NotePatch),
    SetParameter { parameter: Parameter, value: f64 },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Modifier {
    pub id: ModifierId,
    pub name: String,
    pub operations: Vec<Operation>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub id: InstanceId,
    pub name: String,
    pub definition_id: DefinitionId,
    pub track_id: TrackId,
    pub start: Tick,
    pub length: Tick,
    pub modifiers: Vec<ModifierId>,
    pub overrides: Vec<Operation>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub modifier_bindings: BTreeMap<ModifierId, crate::modifier_binding::ModifierBinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modifier_ordering: Option<crate::ordering_graph::ModifierOrdering>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteOrigin {
    pub definition_id: DefinitionId,
    pub note_id: NoteId,
    pub modifier_ids: Vec<ModifierId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub definition_path: Option<Vec<DefinitionPathEntry>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_loop_index: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionPathEntry {
    pub definition_id: DefinitionId,
    pub definition_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement_id: Option<ChildPlacementId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_index: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveNote {
    pub id: NoteId,
    pub start: Tick,
    pub duration: Tick,
    pub pitch: f64,
    pub velocity: f64,
    pub instance_id: InstanceId,
    pub track_id: TrackId,
    pub origin: NoteOrigin,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositionNote {
    pub id: NoteId,
    pub start: Tick,
    pub duration: Tick,
    pub pitch: f64,
    pub velocity: f64,
    pub origin: NoteOrigin,
}
