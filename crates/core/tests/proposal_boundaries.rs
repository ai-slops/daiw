use daiw_core::{
    Acceptance, Operation, Parameter, Principal, ProposalParams, Rejection, accept_proposal,
    create_project, create_proposal, reject_proposal,
};
use std::fmt::Debug;

fn success<T, E: Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("unexpected error: {error:?}"),
    }
}

fn failure<T>(result: Result<T, daiw_core::CoreError>) -> daiw_core::CoreError {
    match result {
        Ok(_) => panic!("unexpected success"),
        Err(error) => error,
    }
}

fn proposal_project(operations: Vec<Operation>) -> daiw_core::Project {
    let mut project = create_project(Some("Proposal boundaries"), true);
    let proposal = success(create_proposal(
        &project,
        ProposalParams {
            expected_revision: project.revision.clone(),
            instance_id: project.instances[0].id.clone(),
            request: "explicit".into(),
            operations: Some(operations),
        },
    ));
    project.proposals.push(proposal);
    project
}

#[test]
fn rejected_proposal_cannot_be_accepted() {
    // Given
    let project = proposal_project(vec![Operation::Transpose { semitones: 12.0 }]);
    let proposal_id = project.proposals[0].id.clone();
    let hunk_id = project.proposals[0].hunks[0].id.clone();
    let rejected = success(reject_proposal(
        &project,
        Principal::Human,
        Rejection {
            expected_revision: project.revision.clone(),
            proposal_id: proposal_id.clone(),
        },
    ));

    // When
    let error = failure(accept_proposal(
        &rejected,
        Principal::Human,
        Acceptance {
            expected_revision: rejected.revision.clone(),
            proposal_id,
            hunk_ids: vec![hunk_id],
        },
    ));

    // Then
    assert_eq!(error.code(), "ProposalClosed");
}

#[test]
fn accepted_hunk_cannot_be_applied_twice() {
    // Given
    let project = proposal_project(vec![
        Operation::Transpose { semitones: 12.0 },
        Operation::Velocity { factor: 0.8 },
    ]);
    let proposal_id = project.proposals[0].id.clone();
    let hunk_id = project.proposals[0].hunks[0].id.clone();
    let first = success(accept_proposal(
        &project,
        Principal::Human,
        Acceptance {
            expected_revision: project.revision.clone(),
            proposal_id: proposal_id.clone(),
            hunk_ids: vec![hunk_id.clone()],
        },
    ));

    // When
    let error = failure(accept_proposal(
        &first.project,
        Principal::Human,
        Acceptance {
            expected_revision: first.project.revision.clone(),
            proposal_id,
            hunk_ids: vec![hunk_id],
        },
    ));

    // Then
    assert_eq!(error.code(), "HunkAlreadyAccepted");
}

#[test]
fn conflicting_combined_override_cannot_enter_head() {
    // Given
    let mut project = create_project(Some("Conflict"), true);
    project.instances[0]
        .overrides
        .push(Operation::SetParameter {
            parameter: Parameter::Pan,
            value: 0.1,
        });
    let proposal = success(create_proposal(
        &project,
        ProposalParams {
            expected_revision: project.revision.clone(),
            instance_id: project.instances[0].id.clone(),
            request: "explicit".into(),
            operations: Some(vec![Operation::SetParameter {
                parameter: Parameter::Pan,
                value: 0.2,
            }]),
        },
    ));
    let proposal_id = proposal.id.clone();
    let hunk_id = proposal.hunks[0].id.clone();
    project.proposals.push(proposal);

    // When
    let error = failure(accept_proposal(
        &project,
        Principal::Human,
        Acceptance {
            expected_revision: project.revision.clone(),
            proposal_id,
            hunk_ids: vec![hunk_id],
        },
    ));

    // Then
    assert_eq!(error.code(), "ModifierConflict");
}

#[test]
fn accepted_proposal_cannot_be_rejected() {
    // Given
    let project = proposal_project(vec![Operation::Transpose { semitones: 12.0 }]);
    let proposal_id = project.proposals[0].id.clone();
    let hunk_id = project.proposals[0].hunks[0].id.clone();
    let accepted = success(accept_proposal(
        &project,
        Principal::Human,
        Acceptance {
            expected_revision: project.revision.clone(),
            proposal_id: proposal_id.clone(),
            hunk_ids: vec![hunk_id],
        },
    ));

    // When
    let error = failure(reject_proposal(
        &accepted.project,
        Principal::Human,
        Rejection {
            expected_revision: accepted.project.revision.clone(),
            proposal_id,
        },
    ));

    // Then
    assert_eq!(error.code(), "ProposalClosed");
}
