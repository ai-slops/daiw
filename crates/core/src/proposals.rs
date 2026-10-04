use crate::error::CoreError;
use crate::ids::{HunkId, ProposalId};
use crate::music::Operation;
use crate::operation_validation::validate_reusable_operations;
use crate::project::Project;
use crate::proposal_adapter::{operation_description, parse_request};
use crate::proposal_model::{
    Acceptance, AcceptanceOutcome, Hunk, Proposal, ProposalAnchor, ProposalParams, ProposalStatus,
    Rejection, SlotBinding,
};
use crate::proposal_rebase::assess;
use crate::protocol::Principal;
use crate::revision::{advance, musical_fingerprint, require_revision};
use crate::validation::validate_project;
use std::collections::BTreeSet;

pub fn create_proposal(project: &Project, params: ProposalParams) -> Result<Proposal, CoreError> {
    require_revision(project, &params.expected_revision)?;
    let instance = project
        .instances
        .iter()
        .find(|instance| instance.id == params.instance_id)
        .ok_or_else(|| CoreError::NotFound(params.instance_id.to_string()))?;
    let definition = project
        .definitions
        .iter()
        .find(|definition| definition.id == instance.definition_id)
        .ok_or_else(|| CoreError::NotFound(instance.definition_id.to_string()))?;
    let operations = match params.operations {
        Some(operations) => operations,
        None => parse_request(&params.request)?,
    };
    if operations.is_empty() {
        return Err(CoreError::UnsupportedCapability(params.request));
    }
    validate_reusable_operations(&operations)?;
    let hunks = operations
        .into_iter()
        .enumerate()
        .map(|(index, operation)| -> Result<Hunk, CoreError> {
            let slot_binding = match &operation {
                Operation::ReplaceSlot { slot, .. } => {
                    let selected = crate::modifier_binding::select_slot(definition, slot)
                        .ok_or_else(|| {
                            CoreError::InvalidProject(format!("unknown note slot {slot}"))
                        })?;
                    let note_ids = selected.note_ids;
                    Some(SlotBinding {
                        slot: slot.clone(),
                        cardinality: note_ids.len(),
                        note_ids,
                        definition_revision: definition.revision,
                        mode: crate::proposal_model::RebaseMode::Strong,
                    })
                }
                Operation::Transpose { .. }
                | Operation::Velocity { .. }
                | Operation::Gain { .. }
                | Operation::AddNotes { .. }
                | Operation::PatchNotes(_)
                | Operation::SetParameter { .. } => None,
            };
            Ok(Hunk {
                id: HunkId::new_random(),
                title: format!("Change {}", index + 1),
                description: operation_description(&operation).to_owned(),
                operation,
                dependencies: Vec::new(),
                accepted: false,
                slot_binding,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Proposal {
        id: ProposalId::new_random(),
        name: "Agent proposal".to_owned(),
        request: params.request,
        base_revision: project.revision.clone(),
        instance_id: params.instance_id,
        status: ProposalStatus::Pending,
        hunks,
        base_anchor: Some(ProposalAnchor {
            definition_id: definition.id.clone(),
            definition_revision: definition.revision,
            note_ids: definition
                .notes
                .iter()
                .map(|note| note.id.clone())
                .collect(),
        }),
        base_fingerprint: Some(musical_fingerprint(project)?),
    })
}

pub fn accept_proposal(
    project: &Project,
    principal: Principal,
    acceptance: Acceptance,
) -> Result<AcceptanceOutcome, CoreError> {
    if principal != Principal::Human {
        return Err(CoreError::PermissionDenied);
    }
    require_revision(project, &acceptance.expected_revision)?;
    let mut changed = project.clone();
    let proposal_index = changed
        .proposals
        .iter()
        .position(|proposal| proposal.id == acceptance.proposal_id)
        .ok_or_else(|| CoreError::NotFound(acceptance.proposal_id.to_string()))?;
    match changed.proposals[proposal_index].status {
        ProposalStatus::Pending | ProposalStatus::PartiallyAccepted => {}
        ProposalStatus::Accepted | ProposalStatus::Rejected => {
            return Err(CoreError::ProposalClosed);
        }
    }
    let selected: BTreeSet<_> = acceptance.hunk_ids.iter().cloned().collect();
    let already_accepted: BTreeSet<_> = changed.proposals[proposal_index]
        .hunks
        .iter()
        .filter(|hunk| hunk.accepted)
        .map(|hunk| hunk.id.clone())
        .collect();
    for hunk in changed.proposals[proposal_index]
        .hunks
        .iter()
        .filter(|hunk| selected.contains(&hunk.id))
    {
        for dependency in &hunk.dependencies {
            if !selected.contains(dependency) && !already_accepted.contains(dependency) {
                return Err(CoreError::MissingDependency(dependency.to_string()));
            }
        }
    }
    if selected.is_empty()
        || selected.iter().any(|id| {
            !changed.proposals[proposal_index]
                .hunks
                .iter()
                .any(|hunk| hunk.id == *id)
        })
    {
        return Err(CoreError::InvalidParams(
            "hunkIds must select known hunks".to_owned(),
        ));
    }
    if let Some(hunk) = changed.proposals[proposal_index]
        .hunks
        .iter()
        .find(|hunk| selected.contains(&hunk.id) && hunk.accepted)
    {
        return Err(CoreError::HunkAlreadyAccepted(hunk.id.to_string()));
    }
    let fingerprint_changed = changed.proposals[proposal_index]
        .base_fingerprint
        .as_ref()
        .map(|base| musical_fingerprint(&changed).map(|current| current != *base))
        .transpose()?
        .unwrap_or(changed.proposals[proposal_index].base_revision != changed.revision);
    let assessment = assess(
        &changed,
        &changed.proposals[proposal_index],
        &selected,
        fingerprint_changed,
    );
    if !assessment.diagnostics.is_empty() {
        let details = assessment
            .diagnostics
            .iter()
            .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
            .collect::<Vec<_>>()
            .join("; ");
        if assessment
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ModifierConflict")
        {
            return Err(CoreError::ModifierConflict(details));
        }
        return Err(CoreError::RebaseConflict(details));
    }
    let operations: Vec<_> = changed.proposals[proposal_index]
        .hunks
        .iter()
        .filter(|hunk| selected.contains(&hunk.id))
        .map(|hunk| hunk.operation.clone())
        .collect();
    let instance_id = changed.proposals[proposal_index].instance_id.clone();
    let instance = changed
        .instances
        .iter_mut()
        .find(|instance| instance.id == instance_id)
        .ok_or_else(|| CoreError::NotFound(instance_id.to_string()))?;
    instance.overrides.extend(operations);
    for hunk in &mut changed.proposals[proposal_index].hunks {
        if selected.contains(&hunk.id) {
            hunk.accepted = true;
        }
    }
    let all_accepted = changed.proposals[proposal_index]
        .hunks
        .iter()
        .all(|hunk| hunk.accepted);
    changed.proposals[proposal_index].status = if all_accepted {
        ProposalStatus::Accepted
    } else {
        ProposalStatus::PartiallyAccepted
    };
    validate_project(&changed)?;
    let project = advance(changed, "human", "Accepted proposal hunks")?;
    Ok(AcceptanceOutcome {
        project,
        accepted: true,
        hunks: assessment.results,
        diagnostics: assessment.diagnostics,
    })
}

pub fn reject_proposal(
    project: &Project,
    principal: Principal,
    rejection: Rejection,
) -> Result<Project, CoreError> {
    if principal != Principal::Human {
        return Err(CoreError::PermissionDenied);
    }
    require_revision(project, &rejection.expected_revision)?;
    let mut changed = project.clone();
    let proposal = changed
        .proposals
        .iter_mut()
        .find(|proposal| proposal.id == rejection.proposal_id)
        .ok_or_else(|| CoreError::NotFound(rejection.proposal_id.to_string()))?;
    match proposal.status {
        ProposalStatus::Pending | ProposalStatus::PartiallyAccepted => {}
        ProposalStatus::Accepted | ProposalStatus::Rejected => {
            return Err(CoreError::ProposalClosed);
        }
    }
    proposal.status = ProposalStatus::Rejected;
    advance(changed, "human", "Rejected proposal")
}
