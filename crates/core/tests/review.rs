use daiw_core::{
    Acceptance, ChangeRequest, Command, EditScope, Operation, Principal, ProposalParams, Request,
    Response, accept_proposal, apply_change, create_project, create_proposal, dispatch,
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

fn success_response(response: Response) -> Box<daiw_core::SuccessResponse> {
    match response {
        Response::Success(response) => response,
        Response::Failure(response) => panic!("unexpected failure: {}", response.error.message),
    }
}

#[test]
fn agent_cannot_create_or_import_project() {
    // Given
    let project = create_project(Some("Imported"), false);
    let create = Request {
        id: "create".into(),
        principal: Principal::Agent,
        state: None,
        method: "project.create".into(),
        params: serde_json::json!({}),
    };
    let import = Request {
        id: "import".into(),
        principal: Principal::Agent,
        state: None,
        method: "project.import".into(),
        params: serde_json::json!({"project": project}),
    };

    // When
    let responses = [dispatch(create), dispatch(import)];

    // Then
    for response in responses {
        match response {
            Response::Failure(response) => assert_eq!(response.error.code, "PermissionDenied"),
            Response::Success(_) => panic!("agent mutation succeeded"),
        }
    }
}

#[test]
fn human_import_validates_and_assigns_new_revision() {
    // Given
    let project = create_project(Some("Imported"), true);
    let old_revision = project.revision.clone();
    let request = Request {
        id: "import".into(),
        principal: Principal::Human,
        state: None,
        method: "project.import".into(),
        params: serde_json::json!({"project": project}),
    };

    // When
    let response = success_response(dispatch(request));

    // Then
    assert_eq!(response.state.name, "Imported");
    assert_ne!(response.state.revision, old_revision);
}

#[test]
fn history_query_returns_canonical_hash_and_revision() {
    // Given
    let project = create_project(Some("History"), true);
    let request = Request {
        id: "history".into(),
        principal: Principal::Human,
        state: Some(project.clone()),
        method: "project.history".into(),
        params: serde_json::json!({}),
    };

    // When
    let response = success_response(dispatch(request));

    // Then
    assert_eq!(response.result["revision"], project.revision.as_str());
    assert_eq!(response.result["hash"].as_str().map(str::len), Some(64));
    assert_eq!(response.result["history"], serde_json::json!([]));
}

#[test]
fn stale_proposal_rebases_after_unrelated_change() {
    // Given
    let mut project = create_project(Some("Rebase"), true);
    let proposal = success(create_proposal(
        &project,
        ProposalParams {
            expected_revision: project.revision.clone(),
            instance_id: project.instances[0].id.clone(),
            request: "octave up".into(),
            operations: None,
        },
    ));
    let proposal_id = proposal.id.clone();
    let hunk_id = proposal.hunks[0].id.clone();
    project.proposals.push(proposal);
    let changed = success(apply_change(
        &project,
        Principal::Human,
        ChangeRequest {
            expected_revision: project.revision.clone(),
            command: Command::RenameProject {
                name: "Renamed".into(),
            },
        },
    ));

    // When
    let outcome = success(accept_proposal(
        &changed,
        Principal::Human,
        Acceptance {
            expected_revision: changed.revision.clone(),
            proposal_id,
            hunk_ids: vec![hunk_id],
        },
    ));

    // Then
    assert!(outcome.accepted);
    assert!(outcome.diagnostics.is_empty());
    assert_eq!(
        outcome.project.proposals[0].status,
        daiw_core::ProposalStatus::Accepted
    );
}

#[test]
fn stale_slot_replacement_returns_typed_rebase_failure() {
    // Given
    let mut project = create_project(Some("Conflict"), true);
    let instance_id = project.instances[0].id.clone();
    let replacement_notes = project.definitions[0].notes.clone();
    let proposal = success(create_proposal(
        &project,
        ProposalParams {
            expected_revision: project.revision.clone(),
            instance_id: instance_id.clone(),
            request: "replace".into(),
            operations: Some(vec![Operation::ReplaceSlot {
                slot: "notes".into(),
                notes: replacement_notes.clone(),
            }]),
        },
    ));
    let proposal_id = proposal.id.clone();
    let hunk_id = proposal.hunks[0].id.clone();
    project.proposals.push(proposal);
    let mut edited_notes = replacement_notes;
    edited_notes[0].pitch += 1.0;
    let changed = success(apply_change(
        &project,
        Principal::Human,
        ChangeRequest {
            expected_revision: project.revision.clone(),
            command: Command::EditNotes {
                instance_id,
                scope: EditScope::Source,
                notes: edited_notes,
            },
        },
    ));

    // When
    let error = failure(accept_proposal(
        &changed,
        Principal::Human,
        Acceptance {
            expected_revision: changed.revision.clone(),
            proposal_id,
            hunk_ids: vec![hunk_id],
        },
    ));

    // Then
    assert_eq!(error.code(), "RebaseConflict");
    assert!(error.to_string().contains("TargetsChanged"));
}
