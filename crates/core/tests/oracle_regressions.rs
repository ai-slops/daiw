use daiw_core::{
    Acceptance, ChangeRequest, ChildPlacement, Command, CoreError, EditScope, HunkId, Note,
    Operation, Principal, Project, ProposalId, ProposalParams, Request, Response, Tick,
    accept_proposal, apply_change, create_project, create_proposal, dispatch, snapshot,
};
use serde_json::json;

fn beat(value: f64) -> Tick {
    Tick::from_beats(value).unwrap_or_else(|error| panic!("invalid test beat: {error}"))
}

fn note(id: &str, pitch: f64) -> Note {
    Note {
        id: id.into(),
        start: beat(0.0),
        duration: beat(0.5),
        pitch,
        velocity: 0.8,
    }
}

fn nested_project() -> daiw_core::Project {
    let mut project = create_project(Some("Oracle regressions"), true);
    project.definitions.truncate(3);
    project.instances.truncate(1);
    project.definitions[0].length = beat(4.0);
    project.definitions[0].slots.clear();
    project.definitions[0].notes = vec![note("root", 60.0)];
    for (index, pitch) in [(1, 64.0), (2, 67.0)] {
        project.definitions[index].length = beat(1.0);
        project.definitions[index].slots.clear();
        project.definitions[index].notes = vec![note(&format!("child-{index}"), pitch)];
    }
    project.definitions[0].children = vec![ChildPlacement {
        id: "child-placement".into(),
        definition_id: project.definitions[1].id.clone(),
        start: beat(1.0),
        length: beat(1.0),
    }];
    project.instances[0].definition_id = project.definitions[0].id.clone();
    project.instances[0].length = beat(4.0);
    project
}

fn stale_add_notes_proposal() -> (Project, ProposalId, HunkId) {
    let mut project = create_project(Some("Stale AddNotes"), true);
    let future = note("future", 72.0);
    let proposal = create_proposal(
        &project,
        ProposalParams {
            expected_revision: project.revision.clone(),
            instance_id: project.instances[0].id.clone(),
            request: "explicit".into(),
            operations: Some(vec![Operation::AddNotes {
                notes: vec![future.clone()],
            }]),
        },
    )
    .unwrap_or_else(|error| panic!("proposal failed: {error:?}"));
    let proposal_id = proposal.id.clone();
    let hunk_id = proposal.hunks[0].id.clone();
    project.proposals.push(proposal);
    let definition_id = project.definitions[0].id.clone();
    let mut source_notes = project.definitions[0].notes.clone();
    source_notes.push(future);
    let changed = apply_change(
        &project,
        Principal::Human,
        ChangeRequest {
            expected_revision: project.revision.clone(),
            command: Command::EditDefinitionNotes {
                definition_id,
                notes: source_notes,
            },
        },
    )
    .unwrap_or_else(|error| panic!("source edit failed: {error:?}"));
    (changed, proposal_id, hunk_id)
}

#[test]
fn legacy_source_edit_checks_overflow_and_unchanged_notes_do_not_increment_definition() {
    // Given
    let project = create_project(Some("Legacy source revision"), true);
    let unchanged_revision = project.definitions[0].revision;
    let unchanged = apply_change(
        &project,
        Principal::Human,
        ChangeRequest {
            expected_revision: project.revision.clone(),
            command: Command::EditNotes {
                instance_id: project.instances[0].id.clone(),
                scope: EditScope::Source,
                notes: project.definitions[0].notes.clone(),
            },
        },
    )
    .unwrap_or_else(|error| panic!("legacy no-op failed: {error:?}"));
    let mut overflow = project.clone();
    overflow.definitions[0].revision = u64::MAX;
    let before = overflow.clone();
    let mut changed_notes = overflow.definitions[0].notes.clone();
    changed_notes[0].pitch += 1.0;

    // When
    let result = apply_change(
        &overflow,
        Principal::Human,
        ChangeRequest {
            expected_revision: overflow.revision.clone(),
            command: Command::EditNotes {
                instance_id: overflow.instances[0].id.clone(),
                scope: EditScope::Source,
                notes: changed_notes,
            },
        },
    );

    // Then
    assert_eq!(unchanged.definitions[0].revision, unchanged_revision);
    assert!(result.is_err());
    assert_eq!(overflow, before);
}

#[test]
fn stale_nested_slot_proposal_cannot_be_previewed() {
    // Given
    let mut project = nested_project();
    let proposal = create_proposal(
        &project,
        ProposalParams {
            expected_revision: project.revision.clone(),
            instance_id: project.instances[0].id.clone(),
            request: "explicit".into(),
            operations: Some(vec![Operation::ReplaceSlot {
                slot: "notes".into(),
                notes: vec![note("replacement", 72.0)],
            }]),
        },
    )
    .unwrap_or_else(|error| panic!("proposal failed: {error:?}"));
    let proposal_id = proposal.id.clone();
    project.proposals.push(proposal);
    let child_id = project.definitions[1].id.clone();
    let changed = apply_change(
        &project,
        Principal::Human,
        ChangeRequest {
            expected_revision: project.revision.clone(),
            command: Command::EditDefinitionNotes {
                definition_id: child_id,
                notes: vec![note("child-1", 65.0)],
            },
        },
    )
    .unwrap_or_else(|error| panic!("descendant edit failed: {error:?}"));

    // When
    let preview = snapshot(&changed, Some(&proposal_id));

    // Then
    assert!(preview.is_err());
}

#[test]
fn stale_add_notes_proposal_cannot_be_previewed_after_source_adds_proposed_id() {
    // Given
    let (project, proposal_id, _) = stale_add_notes_proposal();

    // When
    let result = snapshot(&project, Some(&proposal_id));

    // Then
    assert!(matches!(
        result,
        Err(CoreError::RebaseConflict(details)) if details.contains("TargetsChanged")
    ));
}

#[test]
fn stale_add_notes_proposal_cannot_be_accepted_after_source_adds_proposed_id() {
    // Given
    let (project, proposal_id, hunk_id) = stale_add_notes_proposal();
    let before = project.clone();

    // When
    let result = accept_proposal(
        &project,
        Principal::Human,
        Acceptance {
            expected_revision: project.revision.clone(),
            proposal_id,
            hunk_ids: vec![hunk_id],
        },
    );

    // Then
    assert!(matches!(
        result,
        Err(CoreError::RebaseConflict(details)) if details.contains("TargetsChanged")
    ));
    assert_eq!(project, before);
}

#[test]
fn child_repoint_impact_unions_before_and_after_reachable_definitions() {
    // Given
    let project = nested_project();
    let root = project.definitions[0].id.clone();
    let before_child = project.definitions[1].id.clone();
    let after_child = project.definitions[2].id.clone();
    let mut child = project.definitions[0].children[0].clone();
    child.definition_id = after_child.clone();

    // When
    let response = dispatch(Request {
        id: "impact".into(),
        principal: Principal::Human,
        state: Some(project.clone()),
        method: "change.dryRun".into(),
        params: json!({
            "expectedRevision": project.revision,
            "command": {
                "type": "setDefinitionChild",
                "parentDefinitionId": root,
                "child": child
            }
        }),
    });

    // Then
    let result = match response {
        Response::Success(response) => response.result,
        Response::Failure(response) => panic!("dry run failed: {:?}", response.error),
    };
    let affected = result["impact"]["affectedDefinitions"]
        .as_array()
        .unwrap_or_else(|| panic!("missing affected definitions"));
    assert!(affected.contains(&json!(project.definitions[0].id)));
    assert!(affected.contains(&json!(before_child)));
    assert!(affected.contains(&json!(after_child)));
}
