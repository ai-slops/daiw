use crate::definition_composition::{ComposedNote, WorkMeter};
use crate::error::CoreError;
use crate::ids::{DefinitionId, ModifierId, NoteId};
use crate::music::{Definition, Note, Operation};
use crate::resolution::ResolvedOperation;

pub(crate) struct PatternNote {
    pub note: Note,
    pub definition_id: DefinitionId,
    pub source_note_id: NoteId,
    pub definition_path: Option<Vec<crate::music::DefinitionPathEntry>>,
    pub modifier_ids: Vec<ModifierId>,
}

struct PatternOperation<'a> {
    operation: &'a Operation,
    modifier_id: Option<&'a ModifierId>,
    definition: &'a Definition,
}

pub(crate) fn from_composed(
    composed: Vec<ComposedNote>,
    definition: &Definition,
    operations: &[ResolvedOperation],
    work: &mut WorkMeter,
) -> Result<Vec<PatternNote>, CoreError> {
    let mut pattern: Vec<_> = composed
        .into_iter()
        .map(|item| PatternNote {
            note: item.note,
            definition_id: item.definition_id,
            source_note_id: item.source_note_id,
            definition_path: item.definition_path,
            modifier_ids: Vec::new(),
        })
        .collect();
    for resolved in operations {
        apply(
            &mut pattern,
            PatternOperation {
                operation: &resolved.operation,
                modifier_id: resolved.modifier_id.as_ref(),
                definition,
            },
            work,
        )?;
    }
    Ok(pattern)
}

fn apply(
    notes: &mut Vec<PatternNote>,
    applied: PatternOperation<'_>,
    work: &mut WorkMeter,
) -> Result<(), CoreError> {
    work.consume(operation_work(notes.len(), applied.operation)?)?;
    match applied.operation {
        Operation::Transpose { semitones } => {
            for note in notes {
                note.note.pitch += semitones;
                mark(note, applied.modifier_id);
            }
        }
        Operation::Velocity { factor } => {
            for note in notes {
                note.note.velocity = (note.note.velocity * factor).clamp(0.0, 1.0);
                mark(note, applied.modifier_id);
            }
        }
        Operation::AddNotes { notes: added } => {
            crate::pattern_growth::reserve(notes, added.len())?;
            notes.extend(added.iter().cloned().map(|note| PatternNote {
                definition_id: applied.definition.id.clone(),
                source_note_id: note.id.clone(),
                definition_path: None,
                note,
                modifier_ids: applied.modifier_id.iter().cloned().cloned().collect(),
            }));
        }
        Operation::ReplaceSlot {
            slot,
            notes: replacements,
        } => replace_slot(notes, applied, slot, replacements)?,
        Operation::PatchNotes(patch) => {
            patch.validate_targets(notes.iter().map(|note| &note.note.id))?;
            let removed: std::collections::BTreeSet<_> = patch.removed.iter().collect();
            let updates: std::collections::BTreeMap<_, _> =
                patch.updates.iter().map(|note| (&note.id, note)).collect();
            notes.retain(|note| !removed.contains(&note.note.id));
            for note in notes.iter_mut() {
                if let Some(updated) = updates.get(&note.note.id) {
                    note.note = (*updated).clone();
                }
            }
            crate::pattern_growth::reserve(notes, patch.added.len())?;
            notes.extend(patch.added.iter().cloned().map(|note| PatternNote {
                definition_id: applied.definition.id.clone(),
                source_note_id: note.id.clone(),
                definition_path: None,
                note,
                modifier_ids: Vec::new(),
            }));
        }
        Operation::Gain { .. } | Operation::SetParameter { .. } => {}
    }
    Ok(())
}

fn replace_slot(
    notes: &mut Vec<PatternNote>,
    applied: PatternOperation<'_>,
    slot: &str,
    replacements: &[Note],
) -> Result<(), CoreError> {
    if slot == "notes" {
        notes.clear();
    } else {
        let bounds = applied
            .definition
            .slots
            .get(slot)
            .ok_or_else(|| CoreError::InvalidProject(format!("unknown note slot {slot}")))?;
        if replacements.iter().any(|note| {
            note.start < bounds.start
                || note.start >= bounds.end
                || note.start + note.duration > bounds.end
        }) {
            return Err(CoreError::InvalidProject(format!(
                "replacement notes must fit slot {slot}"
            )));
        }
        notes.retain(|note| note.note.start < bounds.start || note.note.start >= bounds.end);
    }
    crate::pattern_growth::reserve(notes, replacements.len())?;
    notes.extend(replacements.iter().cloned().map(|note| PatternNote {
        definition_id: applied.definition.id.clone(),
        source_note_id: note.id.clone(),
        definition_path: None,
        note,
        modifier_ids: applied.modifier_id.iter().cloned().cloned().collect(),
    }));
    Ok(())
}

fn operation_work(current: usize, operation: &Operation) -> Result<usize, CoreError> {
    let added = match operation {
        Operation::Transpose { .. } | Operation::Velocity { .. } => 0,
        Operation::AddNotes { notes } | Operation::ReplaceSlot { notes, .. } => notes.len(),
        Operation::PatchNotes(patch) => patch
            .removed
            .len()
            .checked_add(patch.updates.len())
            .and_then(|count| count.checked_add(patch.added.len()))
            .ok_or_else(|| CoreError::InvalidProject("pattern operation work overflow".into()))?,
        Operation::Gain { .. } | Operation::SetParameter { .. } => return Ok(0),
    };
    current
        .checked_add(added)
        .ok_or_else(|| CoreError::InvalidProject("pattern operation work overflow".into()))
}

fn mark(note: &mut PatternNote, modifier_id: Option<&ModifierId>) {
    if let Some(id) = modifier_id
        && !note.modifier_ids.contains(id)
    {
        note.modifier_ids.push(id.clone());
    }
}
