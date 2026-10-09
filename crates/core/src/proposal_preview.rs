use crate::definition_composition::WorkMeter;
use crate::error::CoreError;
use crate::ids::ProposalId;
use crate::project::Project;
use crate::proposal_model::ProposalStatus;

pub(crate) fn apply(
    project: &Project,
    proposal_id: Option<&ProposalId>,
    work: &mut WorkMeter,
) -> Result<Project, CoreError> {
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
    let selected = proposal
        .hunks
        .iter()
        .filter(|hunk| !hunk.accepted)
        .map(|hunk| hunk.id.clone())
        .collect();
    let fingerprint_changed = proposal
        .base_fingerprint
        .as_ref()
        .map(|base| crate::revision::musical_fingerprint(project).map(|current| current != *base))
        .transpose()?
        .unwrap_or(proposal.base_revision != project.revision);
    let assessment = crate::proposal_rebase::assess_metered(
        project,
        proposal,
        &selected,
        fingerprint_changed,
        work,
    )?;
    if !assessment.diagnostics.is_empty() {
        return Err(assessment_error(&assessment.diagnostics));
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

fn assessment_error(diagnostics: &[crate::protocol::Diagnostic]) -> CoreError {
    let details = diagnostics
        .iter()
        .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
        .collect::<Vec<_>>()
        .join("; ");
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "ModifierConflict")
    {
        CoreError::ModifierConflict(details)
    } else {
        CoreError::RebaseConflict(details)
    }
}
