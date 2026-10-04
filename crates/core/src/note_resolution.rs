use crate::error::CoreError;
use crate::ids::{ModifierId, TrackId};
use crate::music::{Definition, EffectiveNote, Instance, Note, NoteOrigin, Operation};
use crate::resolution::ResolvedOperation;
use crate::time::Tick;

pub const EFFECTIVE_EVENT_LIMIT: u64 = 100_000;

struct PatternNote {
    note: Note,
    modifier_ids: Vec<ModifierId>,
}

pub struct NoteRequest<'a> {
    pub definition: &'a Definition,
    pub instance: &'a Instance,
    pub track_id: &'a TrackId,
    pub operations: &'a [ResolvedOperation],
    pub event_budget: u64,
}

struct PatternOperation<'a> {
    operation: &'a Operation,
    modifier_id: Option<&'a ModifierId>,
    definition: &'a Definition,
}

struct RepeatRequest<'a> {
    pattern: &'a [PatternNote],
    definition: &'a Definition,
    instance: &'a Instance,
    track_id: &'a TrackId,
    event_budget: u64,
}

pub fn resolve_notes(request: NoteRequest<'_>) -> Result<Vec<EffectiveNote>, CoreError> {
    let mut pattern: Vec<_> = request
        .definition
        .notes
        .iter()
        .cloned()
        .map(|note| PatternNote {
            note,
            modifier_ids: Vec::new(),
        })
        .collect();
    for resolved in request.operations {
        apply_pattern_operation(
            &mut pattern,
            PatternOperation {
                operation: &resolved.operation,
                modifier_id: resolved.modifier_id.as_ref(),
                definition: request.definition,
            },
        )?;
    }
    let notes = repeat_pattern(RepeatRequest {
        pattern: &pattern,
        definition: request.definition,
        instance: request.instance,
        track_id: request.track_id,
        event_budget: request.event_budget,
    })?;
    crate::effective_validation::validate(&notes)?;
    Ok(notes)
}

fn apply_pattern_operation(
    notes: &mut Vec<PatternNote>,
    applied: PatternOperation<'_>,
) -> Result<(), CoreError> {
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
            notes.extend(added.iter().cloned().map(|note| PatternNote {
                note,
                modifier_ids: applied.modifier_id.iter().cloned().cloned().collect(),
            }))
        }
        Operation::ReplaceSlot {
            slot,
            notes: replacements,
        } => {
            if slot == "notes" {
                notes.clear();
            } else {
                let bounds = applied.definition.slots.get(slot).ok_or_else(|| {
                    CoreError::InvalidProject(format!("unknown note slot {slot}"))
                })?;
                if replacements.iter().any(|note| {
                    note.start < bounds.start
                        || note.start >= bounds.end
                        || note.start + note.duration > bounds.end
                }) {
                    return Err(CoreError::InvalidProject(format!(
                        "replacement notes must fit slot {slot}"
                    )));
                }
                notes
                    .retain(|note| note.note.start < bounds.start || note.note.start >= bounds.end);
            }
            notes.extend(replacements.iter().cloned().map(|note| PatternNote {
                note,
                modifier_ids: applied.modifier_id.iter().cloned().cloned().collect(),
            }));
        }
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
            notes.extend(patch.added.iter().cloned().map(|note| PatternNote {
                note,
                modifier_ids: Vec::new(),
            }));
        }
        Operation::Gain { .. } | Operation::SetParameter { .. } => {}
    }
    Ok(())
}

fn repeat_pattern(request: RepeatRequest<'_>) -> Result<Vec<EffectiveNote>, CoreError> {
    let instance_ticks = u64::try_from(request.instance.length.ticks())
        .map_err(|_| CoreError::InvalidProject("instance length must be positive".to_owned()))?;
    let definition_ticks = u64::try_from(request.definition.length.ticks())
        .map_err(|_| CoreError::InvalidProject("definition length must be positive".to_owned()))?;
    if definition_ticks == 0 {
        return Err(CoreError::InvalidProject(
            "definition length must be positive".to_owned(),
        ));
    }
    let repetitions = instance_ticks.div_ceil(definition_ticks);
    let pattern_count = u64::try_from(request.pattern.len()).map_err(|_| {
        CoreError::InvalidProject("pattern note count exceeds platform limits".to_owned())
    })?;
    let event_count = repetitions.checked_mul(pattern_count).ok_or_else(|| {
        CoreError::InvalidProject("effective event budget arithmetic overflow".to_owned())
    })?;
    if event_count > request.event_budget {
        return Err(CoreError::InvalidProject(format!(
            "effective event budget exceeded: {event_count} > {} remaining",
            request.event_budget
        )));
    }
    let capacity = usize::try_from(event_count).map_err(|_| {
        CoreError::InvalidProject("effective event budget exceeds platform limits".to_owned())
    })?;
    let mut output = Vec::with_capacity(capacity);
    let mut loop_start = 0_i64;
    let mut loop_index = 0_u64;
    while loop_start < request.instance.length.ticks() {
        for pattern_note in request.pattern {
            let local_start = loop_start + pattern_note.note.start.ticks();
            if local_start >= request.instance.length.ticks() {
                continue;
            }
            let available = request.instance.length.ticks() - local_start;
            let duration = pattern_note.note.duration.ticks().min(available);
            output.push(EffectiveNote {
                id: format!(
                    "{}@{}#{loop_index}",
                    pattern_note.note.id, request.instance.id
                )
                .into(),
                start: request.instance.start + Tick::from_ticks(local_start),
                duration: Tick::from_ticks(duration),
                pitch: pattern_note.note.pitch,
                velocity: pattern_note.note.velocity,
                instance_id: request.instance.id.clone(),
                track_id: request.track_id.clone(),
                origin: NoteOrigin {
                    definition_id: request.definition.id.clone(),
                    note_id: pattern_note.note.id.clone(),
                    modifier_ids: pattern_note.modifier_ids.clone(),
                },
            });
        }
        loop_start += request.definition.length.ticks();
        loop_index += 1;
    }
    Ok(output)
}

fn mark(note: &mut PatternNote, modifier_id: Option<&ModifierId>) {
    if let Some(id) = modifier_id
        && !note.modifier_ids.contains(id)
    {
        note.modifier_ids.push(id.clone());
    }
}
