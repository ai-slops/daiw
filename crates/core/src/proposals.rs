use crate::error::CoreError;
use crate::project::Project;
use crate::proposal_model::{
    Acceptance, AcceptanceOutcome, Proposal, ProposalParams, ProposalStatus, Rejection,
};
use crate::protocol::Principal;
use crate::revision::{advance, musical_fingerprint, require_revision};
use crate::validation::validate_project;
use std::collections::BTreeSet;

pub fn create_proposal(project: &Project, params: ProposalParams) -> Result<Proposal, CoreError> {
    let mut cache = crate::composition_cache::uncached_session();
    create_proposal_with_session(
        project,
        params,
        &mut crate::composition_cache::ResolutionSession::new(&mut cache),
    )
}

pub(crate) fn create_proposal_with_session(
    project: &Project,
    params: ProposalParams,
    session: &mut crate::composition_cache::ResolutionSession<'_>,
) -> Result<Proposal, CoreError> {
    crate::proposal_creation::create(project, params, session)
}

pub fn accept_proposal(
    project: &Project,
    principal: Principal,
    acceptance: Acceptance,
) -> Result<AcceptanceOutcome, CoreError> {
    let mut cache = crate::composition_cache::uncached_session();
    accept_proposal_with_session(
        project,
        principal,
        acceptance,
        &mut crate::composition_cache::ResolutionSession::new(&mut cache),
    )
}

pub(crate) fn accept_proposal_with_session(
    project: &Project,
    principal: Principal,
    acceptance: Acceptance,
    session: &mut crate::composition_cache::ResolutionSession<'_>,
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
    let assessment = crate::proposal_rebase::assess_metered(
        &changed,
        &changed.proposals[proposal_index],
        &selected,
        fingerprint_changed,
        &mut crate::definition_composition::WorkMeter::new(),
        session,
    )?;
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
