use crate::error::CoreError;
use crate::music::{Operation, Parameter};
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum AccessResource {
    NoteMembership,
    NoteTiming,
    NotePitch,
    NoteVelocity,
    TrackGain,
    Parameter { parameter: Parameter },
    Slot { slot: String },
}

#[derive(Default, Serialize)]
pub struct OperationAccess {
    pub reads: BTreeSet<AccessResource>,
    pub writes: BTreeSet<AccessResource>,
}

pub fn describe(operation: &Operation) -> OperationAccess {
    use AccessResource::{NoteMembership, NotePitch, NoteTiming, NoteVelocity, TrackGain};
    let (reads, writes) = match operation {
        Operation::Transpose { .. } => (vec![NoteMembership, NotePitch], vec![NotePitch]),
        Operation::Velocity { .. } => (vec![NoteMembership, NoteVelocity], vec![NoteVelocity]),
        Operation::Gain { .. } => (vec![TrackGain], vec![TrackGain]),
        Operation::AddNotes { .. } => (
            vec![],
            vec![NoteMembership, NoteTiming, NotePitch, NoteVelocity],
        ),
        Operation::ReplaceSlot { slot, .. } => (
            vec![
                NoteMembership,
                NoteTiming,
                AccessResource::Slot { slot: slot.clone() },
            ],
            vec![NoteMembership, NoteTiming, NotePitch, NoteVelocity],
        ),
        Operation::PatchNotes(_) => (
            vec![NoteMembership, NoteTiming, NotePitch, NoteVelocity],
            vec![NoteMembership, NoteTiming, NotePitch, NoteVelocity],
        ),
        Operation::SetParameter { parameter, .. } => (
            vec![],
            vec![AccessResource::Parameter {
                parameter: parameter.clone(),
            }],
        ),
    };
    OperationAccess {
        reads: reads.into_iter().collect(),
        writes: writes.into_iter().collect(),
    }
}

pub fn validate_additions<'a>(
    operations: impl Iterator<Item = &'a Operation>,
) -> Result<(), CoreError> {
    let mut added_ids = BTreeSet::new();
    for operation in operations {
        if let Operation::AddNotes { notes } = operation {
            for note in notes {
                if !added_ids.insert(&note.id) {
                    return Err(CoreError::ModifierConflict(format!(
                        "added note:{}",
                        note.id
                    )));
                }
            }
        }
    }
    Ok(())
}

pub fn requires_order(left: &Operation, right: &Operation) -> Result<bool, CoreError> {
    let a = describe(left);
    let b = describe(right);
    let hazard = !a.writes.is_disjoint(&b.reads)
        || !a.writes.is_disjoint(&b.writes)
        || !b.writes.is_disjoint(&a.reads);
    match left {
        Operation::Gain { .. } => match right {
            Operation::Gain { .. } => Ok(false),
            Operation::Transpose { .. }
            | Operation::Velocity { .. }
            | Operation::AddNotes { .. }
            | Operation::ReplaceSlot { .. }
            | Operation::PatchNotes(_)
            | Operation::SetParameter { .. } => Ok(hazard),
        },
        Operation::AddNotes { notes } => match right {
            Operation::AddNotes { notes: other } => {
                let ids: BTreeSet<_> = notes.iter().map(|note| &note.id).collect();
                if let Some(note) = other.iter().find(|note| ids.contains(&note.id)) {
                    Err(CoreError::ModifierConflict(format!(
                        "added note:{}",
                        note.id
                    )))
                } else {
                    Ok(false)
                }
            }
            Operation::Transpose { .. }
            | Operation::Velocity { .. }
            | Operation::Gain { .. }
            | Operation::ReplaceSlot { .. }
            | Operation::PatchNotes(_)
            | Operation::SetParameter { .. } => Ok(hazard),
        },
        Operation::ReplaceSlot { slot, .. } => match right {
            Operation::ReplaceSlot { slot: other, .. } => {
                if slot == other {
                    Err(CoreError::ModifierConflict(format!("slot:{slot}")))
                } else {
                    Ok(hazard)
                }
            }
            Operation::Transpose { .. }
            | Operation::Velocity { .. }
            | Operation::Gain { .. }
            | Operation::AddNotes { .. }
            | Operation::PatchNotes(_)
            | Operation::SetParameter { .. } => Ok(hazard),
        },
        Operation::SetParameter { parameter, .. } => match right {
            Operation::SetParameter {
                parameter: other, ..
            } => {
                if parameter == other {
                    Err(CoreError::ModifierConflict(format!(
                        "parameter:{parameter:?}"
                    )))
                } else {
                    Ok(false)
                }
            }
            Operation::Transpose { .. }
            | Operation::Velocity { .. }
            | Operation::Gain { .. }
            | Operation::AddNotes { .. }
            | Operation::ReplaceSlot { .. }
            | Operation::PatchNotes(_) => Ok(hazard),
        },
        Operation::Transpose { .. } | Operation::Velocity { .. } | Operation::PatchNotes(_) => {
            Ok(hazard)
        }
    }
}
