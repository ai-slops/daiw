use crate::error::CoreError;
use crate::ids::{NoteId, TrackId};
use crate::music::{Definition, EffectiveNote, Instance, Note, NoteOrigin};
use crate::pattern_resolution::PatternNote;
use crate::resolution::ResolvedOperation;
use crate::time::Tick;

pub const EFFECTIVE_EVENT_LIMIT: u64 = 100_000;

pub struct NoteRequest<'a> {
    pub definition: &'a Definition,
    pub instance: &'a Instance,
    pub track_id: &'a TrackId,
    pub operations: &'a [ResolvedOperation],
    pub event_budget: u64,
}

pub(crate) struct EditableRequest<'a> {
    pub definition: &'a Definition,
    pub operations: &'a [ResolvedOperation],
    pub instance_id: &'a crate::ids::InstanceId,
    pub length: Tick,
}

struct RepeatRequest<'a> {
    pattern: &'a [PatternNote],
    definition: &'a Definition,
    instance: &'a Instance,
    track_id: &'a TrackId,
    event_budget: u64,
}

pub(crate) fn resolve_composed(
    request: NoteRequest<'_>,
    composed: Vec<crate::definition_composition::ComposedNote>,
    work: &mut crate::definition_composition::WorkMeter,
) -> Result<Vec<EffectiveNote>, CoreError> {
    let pattern = crate::pattern_resolution::from_composed(
        composed,
        request.definition,
        request.operations,
        work,
    )?;
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

pub(crate) fn editable_from_composed(
    request: EditableRequest<'_>,
    composed: Vec<crate::definition_composition::ComposedNote>,
    work: &mut crate::definition_composition::WorkMeter,
) -> Result<Vec<Note>, CoreError> {
    let limit = request.definition.length.min(request.length);
    let notes = crate::pattern_resolution::from_composed(
        composed,
        request.definition,
        request.operations,
        work,
    )?
    .into_iter()
    .filter_map(|mut item| {
        if item.note.start >= limit {
            return None;
        }
        item.note.duration = item
            .note
            .duration
            .min(Tick::from_ticks(limit.ticks() - item.note.start.ticks()));
        Some(item.note)
    })
    .collect::<Vec<_>>();
    crate::validation::validate_notes(&notes)?;
    crate::note_patch::validate_bounds(&notes, limit)?;
    validate_editable_ids(request.definition, request.instance_id, &notes)?;
    Ok(notes)
}

fn validate_editable_ids(
    definition: &Definition,
    instance_id: &crate::ids::InstanceId,
    notes: &[Note],
) -> Result<(), CoreError> {
    let mut ids = std::collections::BTreeSet::new();
    for note in notes {
        let id = if definition.children.is_empty() {
            NoteId::from(format!("{}@{instance_id}#0", note.id))
        } else {
            crate::note_identity::nested_effective(instance_id, 0, &note.id)?
        };
        if !ids.insert(id) {
            return Err(CoreError::InvalidProject(
                "effective note id collision in editable pattern".into(),
            ));
        }
    }
    Ok(())
}

fn repeat_pattern(request: RepeatRequest<'_>) -> Result<Vec<EffectiveNote>, CoreError> {
    if request.pattern.is_empty() {
        return Ok(Vec::new());
    }
    if request.definition.length.ticks() <= 0 {
        return Err(CoreError::InvalidProject(
            "definition length must be positive".to_owned(),
        ));
    }
    let mut output = Vec::new();
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
            let emitted = u64::try_from(output.len()).map_err(|_| {
                CoreError::InvalidProject("effective event count exceeds platform limits".into())
            })?;
            if emitted >= request.event_budget {
                return Err(CoreError::InvalidProject(format!(
                    "effective event budget exceeded: more than {} remaining",
                    request.event_budget
                )));
            }
            output.push(EffectiveNote {
                id: if request.definition.children.is_empty() {
                    format!(
                        "{}@{}#{loop_index}",
                        pattern_note.note.id, request.instance.id
                    )
                    .into()
                } else {
                    crate::note_identity::nested_effective(
                        &request.instance.id,
                        loop_index,
                        &pattern_note.note.id,
                    )?
                },
                start: request.instance.start + Tick::from_ticks(local_start),
                duration: Tick::from_ticks(duration),
                pitch: pattern_note.note.pitch,
                velocity: pattern_note.note.velocity,
                instance_id: request.instance.id.clone(),
                track_id: request.track_id.clone(),
                origin: NoteOrigin {
                    definition_id: pattern_note.definition_id.clone(),
                    note_id: pattern_note.source_note_id.clone(),
                    modifier_ids: pattern_note.modifier_ids.clone(),
                    definition_path: pattern_note.definition_path.clone(),
                    instance_loop_index: pattern_note.definition_path.as_ref().map(|_| loop_index),
                },
            });
        }
        loop_start = loop_start
            .checked_add(request.definition.length.ticks())
            .ok_or_else(|| CoreError::InvalidProject("effective event timing overflow".into()))?;
        loop_index = loop_index.checked_add(1).ok_or_else(|| {
            CoreError::InvalidProject("effective event loop index overflow".into())
        })?;
    }
    Ok(output)
}
