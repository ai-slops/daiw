use crate::error::CoreError;
use crate::ids::{InstanceId, ProposalId};
use crate::instance_resolution::InstanceResolution;
use crate::music::Note;
use crate::note_resolution::{EFFECTIVE_EVENT_LIMIT, NoteRequest};
use crate::project::{Project, Snapshot};
use crate::track_resolution::derive_track;
use crate::validation::validate_project;

pub fn snapshot(
    project: &Project,
    proposal_id: Option<&ProposalId>,
) -> Result<Snapshot, CoreError> {
    let mut cache = crate::composition_cache::uncached_session();
    snapshot_with_session(
        project,
        proposal_id,
        &mut crate::composition_cache::ResolutionSession::new(&mut cache),
    )
}

pub(crate) fn snapshot_with_session(
    project: &Project,
    proposal_id: Option<&ProposalId>,
    session: &mut crate::composition_cache::ResolutionSession<'_>,
) -> Result<Snapshot, CoreError> {
    let mut work = crate::definition_composition::WorkMeter::new();
    let rendered = crate::proposal_preview::apply(project, proposal_id, &mut work, session)?;
    validate_project(&rendered)?;
    let mut tracks = rendered.tracks.clone();
    let mut notes = Vec::new();
    for instance in &rendered.instances {
        let definition = rendered
            .definitions
            .iter()
            .find(|definition| definition.id == instance.definition_id)
            .ok_or_else(|| CoreError::NotFound(instance.definition_id.to_string()))?;
        let base_track = rendered
            .tracks
            .iter()
            .find(|track| track.id == instance.track_id)
            .ok_or_else(|| CoreError::NotFound(instance.track_id.to_string()))?;
        let composed = session.compose(&rendered, definition, &mut work)?;
        let operations = InstanceResolution {
            project: &rendered,
            instance,
            definition,
            composed: &composed,
        }
        .validated_operations()?;
        let track_id = match derive_track(base_track, &instance.id, &operations)? {
            Some(track) => {
                let id = track.id.clone();
                if tracks.iter().any(|existing| existing.id == id) {
                    return Err(CoreError::InvalidProject(format!(
                        "effective track id collides for instance {}",
                        instance.id
                    )));
                }
                tracks.push(track);
                id
            }
            None => base_track.id.clone(),
        };
        let used_events = u64::try_from(notes.len()).map_err(|_| {
            CoreError::InvalidProject("effective event count exceeds platform limits".to_owned())
        })?;
        let event_budget = EFFECTIVE_EVENT_LIMIT
            .checked_sub(used_events)
            .ok_or_else(|| {
                CoreError::InvalidProject("effective event budget exhausted".to_owned())
            })?;
        let pattern = session.pattern(
            crate::instance_pattern_cache::PatternRequest {
                project: &rendered,
                instance,
                definition,
                composed,
                operations: &operations,
            },
            &mut work,
        )?;
        notes.extend(crate::note_resolution::resolve_pattern(
            NoteRequest {
                definition,
                instance,
                track_id: &track_id,
                event_budget,
            },
            &pattern,
        )?);
    }
    crate::effective_validation::validate(&notes)?;
    notes.sort_by(|left, right| {
        left.start
            .cmp(&right.start)
            .then_with(|| left.id.cmp(&right.id))
    });
    let instance_end = rendered
        .instances
        .iter()
        .map(|instance| instance.start + instance.length)
        .max()
        .unwrap_or_default();
    let region_end = rendered
        .regions
        .iter()
        .map(|region| region.start + region.duration)
        .max()
        .unwrap_or_default();
    Ok(Snapshot {
        revision: project.revision.clone(),
        tempo: project.tempo,
        tempo_map: project.tempo_map.clone(),
        meter: Some(project.meter),
        meter_map: project.meter_map.clone(),
        length: instance_end.max(region_end),
        tracks,
        notes,
        regions: project.regions.clone(),
    })
}

pub(crate) fn editable_notes_metered_with_session(
    project: &Project,
    id: &InstanceId,
    work: &mut crate::definition_composition::WorkMeter,
    session: &mut crate::composition_cache::ResolutionSession<'_>,
) -> Result<Vec<Note>, CoreError> {
    let instance = project
        .instances
        .iter()
        .find(|instance| instance.id == *id)
        .ok_or_else(|| CoreError::NotFound(id.to_string()))?;
    let definition = project
        .definitions
        .iter()
        .find(|definition| definition.id == instance.definition_id)
        .ok_or_else(|| CoreError::NotFound(instance.definition_id.to_string()))?;
    let composed = session.compose(project, definition, work)?;
    let operations = InstanceResolution {
        project,
        instance,
        definition,
        composed: &composed,
    }
    .validated_operations()?;
    let pattern = session.pattern(
        crate::instance_pattern_cache::PatternRequest {
            project,
            instance,
            definition,
            composed,
            operations: &operations,
        },
        work,
    )?;
    crate::note_resolution::editable_from_pattern(
        crate::note_resolution::EditableRequest {
            definition,
            instance_id: &instance.id,
            length: instance.length,
        },
        pattern,
    )
}

pub(crate) fn explain_notes_with_session(
    project: &Project,
    id: &InstanceId,
    session: &mut crate::composition_cache::ResolutionSession<'_>,
) -> Result<(Vec<Note>, Vec<crate::music::CompositionNote>), CoreError> {
    let mut work = crate::definition_composition::WorkMeter::new();
    let instance = project
        .instances
        .iter()
        .find(|instance| instance.id == *id)
        .ok_or_else(|| CoreError::NotFound(id.to_string()))?;
    let definition = project
        .definitions
        .iter()
        .find(|definition| definition.id == instance.definition_id)
        .ok_or_else(|| CoreError::NotFound(instance.definition_id.to_string()))?;
    let composed = session.compose(project, definition, &mut work)?;
    let composition = composed
        .iter()
        .cloned()
        .map(|item| crate::music::CompositionNote {
            id: item.note.id,
            start: item.note.start,
            duration: item.note.duration,
            pitch: item.note.pitch,
            velocity: item.note.velocity,
            origin: crate::music::NoteOrigin {
                definition_id: item.definition_id,
                note_id: item.source_note_id,
                modifier_ids: Vec::new(),
                definition_path: item.definition_path,
                instance_loop_index: None,
            },
        })
        .collect();
    let operations = InstanceResolution {
        project,
        instance,
        definition,
        composed: &composed,
    }
    .validated_operations()?;
    let pattern = session.pattern(
        crate::instance_pattern_cache::PatternRequest {
            project,
            instance,
            definition,
            composed,
            operations: &operations,
        },
        &mut work,
    )?;
    let editable = crate::note_resolution::editable_from_pattern(
        crate::note_resolution::EditableRequest {
            definition,
            instance_id: &instance.id,
            length: instance.length,
        },
        pattern,
    )?;
    Ok((editable, composition))
}
