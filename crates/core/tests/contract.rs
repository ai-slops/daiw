use daiw_core::{
    Acceptance, ChangeRequest, Command, EditScope, Hunk, Operation, Principal, ProposalParams,
    Request, Response, Tick, accept_proposal, apply_change, create_project, create_proposal,
    snapshot,
};
use std::fmt::Debug;

fn success<T, E: Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("unexpected error: {error:?}"),
    }
}

fn failure<T, E>(result: Result<T, E>) -> E {
    match result {
        Ok(_) => panic!("unexpected success"),
        Err(error) => error,
    }
}

fn present<T>(value: Option<T>) -> T {
    match value {
        Some(value) => value,
        None => panic!("expected value"),
    }
}

fn fixture() -> daiw_core::Project {
    create_project(Some("Fixture"), true)
}

#[test]
fn fixture_has_linked_choruses_when_created() {
    // Given
    let project = fixture();

    // When
    let chorus_definition = present(
        project
            .definitions
            .iter()
            .find(|definition| definition.id.as_str() == "def-chorus"),
    );

    // Then
    assert_eq!(
        project
            .instances
            .iter()
            .filter(|instance| instance.definition_id == chorus_definition.id)
            .count(),
        3
    );
    assert_eq!(project.tracks.len(), 5);
    assert_eq!(
        success(snapshot(&project, None)).length,
        success(Tick::from_beats(128.0))
    );
}

#[test]
fn source_note_edit_propagates_to_linked_instances() {
    // Given
    let project = fixture();
    let expected = project.revision.clone();
    let instance_id = project.instances[0].id.clone();
    let mut notes = project.definitions[0].notes.clone();
    notes[0].pitch = 73.0;

    // When
    let changed = success(apply_change(
        &project,
        Principal::Human,
        ChangeRequest {
            expected_revision: expected,
            command: Command::EditNotes {
                instance_id,
                scope: EditScope::Source,
                notes,
            },
        },
    ));

    // Then
    let notes = success(snapshot(&changed, None)).notes;
    assert!(
        notes
            .iter()
            .filter(|note| note.origin.definition_id.as_str() == "def-chorus")
            .all(|note| {
                note.pitch == 73.0 || note.origin.note_id.as_str() != "note-chorus-root"
            })
    );
}

#[test]
fn instance_note_edit_preserves_source_provenance() {
    // Given
    let project = fixture();
    let expected = project.revision.clone();
    let instance_id = project.instances[0].id.clone();
    let mut notes = project.definitions[0].notes.clone();
    notes[0].pitch = 49.0;

    // When
    let changed = success(apply_change(
        &project,
        Principal::Human,
        ChangeRequest {
            expected_revision: expected,
            command: Command::EditNotes {
                instance_id: instance_id.clone(),
                scope: EditScope::Instance,
                notes,
            },
        },
    ));

    // Then
    let rendered = success(snapshot(&changed, None));
    let local = present(
        rendered
            .notes
            .iter()
            .find(|note| note.instance_id == instance_id),
    );
    assert_eq!(local.origin.definition_id.as_str(), "def-chorus");
    assert_eq!(local.origin.note_id.as_str(), "note-chorus-root");
    assert_eq!(local.pitch, 49.0);
}

#[test]
fn conflicting_modifier_operations_are_rejected() {
    // Given
    let mut project = fixture();
    project.modifiers.push(daiw_core::Modifier {
        id: "mod-conflict".into(),
        name: "Conflict".into(),
        operations: vec![
            Operation::SetParameter {
                parameter: daiw_core::Parameter::Cutoff,
                value: 900.0,
            },
            Operation::SetParameter {
                parameter: daiw_core::Parameter::Cutoff,
                value: 1200.0,
            },
        ],
    });
    project.instances[0].modifiers.push("mod-conflict".into());

    // When
    let error = failure(snapshot(&project, None));

    // Then
    assert_eq!(error.code(), "ModifierConflict");
}

#[test]
fn stale_and_agent_apply_are_rejected_without_mutating_head() {
    // Given
    let project = fixture();

    // When
    let stale = apply_change(
        &project,
        Principal::Human,
        ChangeRequest {
            expected_revision: "stale".into(),
            command: Command::RenameProject {
                name: "Wrong".into(),
            },
        },
    );
    let agent = apply_change(
        &project,
        Principal::Agent,
        ChangeRequest {
            expected_revision: project.revision.clone(),
            command: Command::RenameProject {
                name: "Wrong".into(),
            },
        },
    );

    // Then
    assert_eq!(failure(stale).code(), "StaleRevision");
    assert_eq!(failure(agent).code(), "PermissionDenied");
    assert_eq!(project.name, "Fixture");
}

#[test]
fn partial_acceptance_requires_dependency_closure() {
    // Given
    let project = fixture();
    let instance_id = project.instances[0].id.clone();
    let proposal = success(create_proposal(
        &project,
        ProposalParams {
            expected_revision: project.revision.clone(),
            instance_id,
            request: "explicit".into(),
            operations: Some(vec![
                Operation::Transpose { semitones: 12.0 },
                Operation::Velocity { factor: 0.8 },
            ]),
        },
    ));
    let mut staged = project.clone();
    let dependency = proposal.hunks[0].id.clone();
    let dependent = proposal.hunks[1].id.clone();
    staged.proposals.push(daiw_core::Proposal {
        hunks: vec![
            proposal.hunks[0].clone(),
            Hunk {
                dependencies: vec![dependency.clone()],
                ..proposal.hunks[1].clone()
            },
        ],
        ..proposal
    });

    // When
    let rejected = accept_proposal(
        &staged,
        Principal::Human,
        Acceptance {
            expected_revision: staged.revision.clone(),
            proposal_id: staged.proposals[0].id.clone(),
            hunk_ids: vec![dependent],
        },
    );
    let accepted = accept_proposal(
        &staged,
        Principal::Human,
        Acceptance {
            expected_revision: staged.revision.clone(),
            proposal_id: staged.proposals[0].id.clone(),
            hunk_ids: vec![dependency],
        },
    );

    // Then
    assert_eq!(failure(rejected).code(), "MissingDependency");
    assert_eq!(
        success(accepted).project.proposals[0].status,
        daiw_core::ProposalStatus::PartiallyAccepted
    );
}

#[test]
fn request_round_trip_returns_state_on_success() {
    // Given
    let request = Request {
        id: "rpc-1".into(),
        principal: Principal::Human,
        state: None,
        method: "project.create".into(),
        params: serde_json::json!({"name":"Wire","fixture":false}),
    };

    // When
    let response = daiw_core::dispatch(request);

    // Then
    match response {
        Response::Success(success) => assert_eq!(success.state.name, "Wire"),
        Response::Failure(failure) => panic!("unexpected failure: {}", failure.error.message),
    }
}
