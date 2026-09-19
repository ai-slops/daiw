use crate::error::CoreError;
use crate::ids::ProposalId;
use crate::note_resolution::{EFFECTIVE_EVENT_LIMIT, NoteRequest, resolve_notes};
use crate::project::{Project, Snapshot};
use crate::proposal_model::ProposalStatus;
use crate::resolution::ResolvedOperation;
use crate::track_resolution::derive_track;
use crate::validation::{validate_operations, validate_project};

pub fn snapshot(
    project: &Project,
    proposal_id: Option<&ProposalId>,
) -> Result<Snapshot, CoreError> {
    let rendered = with_preview(project, proposal_id)?;
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
        let operations = operations_for_instance(&rendered, instance)?;
        validate_operations(
            &operations
                .iter()
                .map(|resolved| resolved.operation.clone())
                .collect::<Vec<_>>(),
        )?;
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
        notes.extend(resolve_notes(NoteRequest {
            definition,
            instance,
            track_id: &track_id,
            operations: &operations,
            event_budget,
        })?);
    }
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
        length: instance_end.max(region_end),
        tracks,
        notes,
        regions: project.regions.clone(),
    })
}

fn with_preview(project: &Project, proposal_id: Option<&ProposalId>) -> Result<Project, CoreError> {
    let mut rendered = project.clone();
    let Some(id) = proposal_id else {
        return Ok(rendered);
    };
    let proposal = project
        .proposals
        .iter()
        .find(|proposal| proposal.id == *id)
        .ok_or_else(|| CoreError::NotFound(id.to_string()))?;
    match proposal.status {
        ProposalStatus::Pending | ProposalStatus::PartiallyAccepted => {}
        ProposalStatus::Accepted | ProposalStatus::Rejected => {
            return Err(CoreError::ProposalClosed);
        }
    }
    let instance = rendered
        .instances
        .iter_mut()
        .find(|instance| instance.id == proposal.instance_id)
        .ok_or_else(|| CoreError::NotFound(proposal.instance_id.to_string()))?;
    instance.overrides.extend(
        proposal
            .hunks
            .iter()
            .filter(|hunk| !hunk.accepted)
            .map(|hunk| hunk.operation.clone()),
    );
    Ok(rendered)
}

fn operations_for_instance(
    project: &Project,
    instance: &crate::music::Instance,
) -> Result<Vec<ResolvedOperation>, CoreError> {
    let mut operations = Vec::new();
    for modifier_id in &instance.modifiers {
        let modifier = project
            .modifiers
            .iter()
            .find(|modifier| modifier.id == *modifier_id)
            .ok_or_else(|| CoreError::NotFound(modifier_id.to_string()))?;
        operations.extend(
            modifier
                .operations
                .iter()
                .cloned()
                .map(|operation| ResolvedOperation {
                    operation,
                    modifier_id: Some(modifier_id.clone()),
                }),
        );
    }
    operations.extend(
        instance
            .overrides
            .iter()
            .cloned()
            .map(|operation| ResolvedOperation {
                operation,
                modifier_id: None,
            }),
    );
    Ok(operations)
}
