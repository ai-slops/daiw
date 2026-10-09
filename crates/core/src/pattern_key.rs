use crate::error::CoreError;
use crate::music::{Definition, Note, Operation, Parameter};
use crate::project::Project;
use crate::resolution::ResolvedOperation;

const KEY_DOMAIN: &[u8] = b"daiw:instance-pattern-cache:v1\0";

pub(crate) fn pattern_key(
    project: &Project,
    definition: &Definition,
    instance_id: &crate::ids::InstanceId,
    operations: &[ResolvedOperation],
) -> Result<[u8; 32], CoreError> {
    let dependency = crate::composition_key::composition_key(project, definition)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(KEY_DOMAIN);
    hasher.update(&dependency);
    hash_str(&mut hasher, instance_id.as_str());
    hasher.update(&definition.slots.len().to_le_bytes());
    for (name, slot) in &definition.slots {
        hash_str(&mut hasher, name);
        hasher.update(&slot.start.ticks().to_le_bytes());
        hasher.update(&slot.end.ticks().to_le_bytes());
    }
    hasher.update(&operations.len().to_le_bytes());
    for resolved in operations {
        match &resolved.modifier_id {
            Some(id) => {
                hasher.update(&[1]);
                hash_str(&mut hasher, id.as_str());
            }
            None => {
                hasher.update(&[0]);
            }
        }
        hash_operation(&mut hasher, &resolved.operation);
    }
    Ok(*hasher.finalize().as_bytes())
}

fn hash_operation(hasher: &mut blake3::Hasher, operation: &Operation) {
    match operation {
        Operation::Transpose { semitones } => hash_float(hasher, 0, *semitones),
        Operation::Velocity { factor } => hash_float(hasher, 1, *factor),
        Operation::Gain { db } => hash_float(hasher, 2, *db),
        Operation::AddNotes { notes } => {
            hasher.update(&[3]);
            hash_notes(hasher, notes);
        }
        Operation::ReplaceSlot { slot, notes } => {
            hasher.update(&[4]);
            hash_str(hasher, slot);
            hash_notes(hasher, notes);
        }
        Operation::PatchNotes(patch) => {
            hasher.update(&[5]);
            hash_notes(hasher, &patch.updates);
            hash_notes(hasher, &patch.added);
            hasher.update(&patch.removed.len().to_le_bytes());
            for id in &patch.removed {
                hash_str(hasher, id.as_str());
            }
        }
        Operation::SetParameter { parameter, value } => {
            hasher.update(&[6]);
            hasher.update(&[match parameter {
                Parameter::Cutoff => 0,
                Parameter::Pan => 1,
            }]);
            hasher.update(&value.to_bits().to_le_bytes());
        }
    }
}

fn hash_notes(hasher: &mut blake3::Hasher, notes: &[Note]) {
    hasher.update(&notes.len().to_le_bytes());
    for note in notes {
        hash_str(hasher, note.id.as_str());
        hasher.update(&note.start.ticks().to_le_bytes());
        hasher.update(&note.duration.ticks().to_le_bytes());
        hasher.update(&note.pitch.to_bits().to_le_bytes());
        hasher.update(&note.velocity.to_bits().to_le_bytes());
    }
}

fn hash_float(hasher: &mut blake3::Hasher, tag: u8, value: f64) {
    hasher.update(&[tag]);
    hasher.update(&value.to_bits().to_le_bytes());
}

fn hash_str(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(&value.len().to_le_bytes());
    hasher.update(value.as_bytes());
}
