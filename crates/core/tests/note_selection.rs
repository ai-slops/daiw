use daiw_core::{
    ChangeRequest, Command, EditScope, NoteSelector, NoteTransform, Principal, Project,
    QueryNotesParams, Request, Response, SelectionHandle, apply_change, create_project,
    query_notes, snapshot,
};

fn success<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
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

fn query(project: &Project, scope: EditScope, selector: NoteSelector) -> SelectionHandle {
    success(query_notes(
        project,
        QueryNotesParams {
            instance_id: project.instances[0].id.clone(),
            scope,
            selector,
            max_results: 16,
        },
    ))
}

fn transform(
    project: &Project,
    selection: SelectionHandle,
    transform: NoteTransform,
) -> Result<Project, daiw_core::CoreError> {
    apply_change(
        project,
        Principal::Human,
        ChangeRequest {
            expected_revision: project.revision.clone(),
            command: Command::TransformSelectedNotes {
                selection,
                transform,
            },
        },
    )
}

#[test]
fn deterministic_bounded_selectors_resolve_stable_ids() {
    // Given: a linked source with a named slot and four stable note IDs.
    let project = create_project(Some("Selectors"), true);
    let selectors = [
        NoteSelector::All,
        NoteSelector::StableIds {
            note_ids: vec!["note-chorus-two".into()],
        },
        NoteSelector::NamedSlot {
            slot: "ending-fill".into(),
        },
        NoteSelector::BeatRange {
            start: daiw_core::Tick::from_ticks(1_920),
            end: daiw_core::Tick::from_ticks(5_760),
        },
        NoteSelector::PitchRange {
            min: 64.0,
            max: 68.0,
        },
    ];

    // When: each bounded selector is resolved twice.
    let resolved: Vec<_> = selectors
        .into_iter()
        .map(|selector| {
            let first = query(&project, EditScope::Source, selector.clone());
            let second = query(&project, EditScope::Source, selector);
            assert_eq!(first, second);
            first
        })
        .collect();

    // Then: ordering is stable and every handle is revision/cardinality/fingerprint bound.
    assert_eq!(resolved[0].note_ids.len(), 4);
    assert_eq!(resolved[1].note_ids[0].as_str(), "note-chorus-two");
    assert_eq!(resolved[2].note_ids[0].as_str(), "note-chorus-four");
    assert_eq!(resolved[3].note_ids.len(), 2);
    assert_eq!(resolved[4].note_ids.len(), 3);
    assert!(
        resolved
            .iter()
            .all(|handle| handle.cardinality == handle.note_ids.len()
                && !handle.resolution_fingerprint.is_empty())
    );

    let too_small = query_notes(
        &project,
        QueryNotesParams {
            instance_id: project.instances[0].id.clone(),
            scope: EditScope::Source,
            selector: NoteSelector::All,
            max_results: 3,
        },
    );
    assert_eq!(failure(too_small).code(), "SelectionLimitExceeded");
}

#[test]
fn query_notes_is_available_through_the_worker_contract() {
    // Given: an agent query for whole notes.
    let project = create_project(Some("Wire selection"), true);
    let request = Request {
        id: "query-1".into(),
        principal: Principal::Agent,
        state: Some(project.clone()),
        method: "project.queryNotes".into(),
        params: serde_json::json!({"instanceId":project.instances[0].id,"scope":"source","selector":{"type":"all"},"maxResults":16}),
    };

    // When: the request is dispatched through the real JSON worker surface.
    let response = daiw_core::dispatch(request);

    // Then: it returns the selection while preserving canonical state.
    match response {
        Response::Success(response) => {
            assert_eq!(response.state, project);
            assert_eq!(response.result["selection"]["cardinality"], 4);
        }
        Response::Failure(response) => panic!("unexpected failure: {}", response.error.message),
    }
}

#[test]
fn source_transform_changes_only_selected_note_across_linked_instances_and_round_trips() {
    // Given: one selected source note shared by three instances.
    let project = create_project(Some("Source target"), true);
    let selection = query(
        &project,
        EditScope::Source,
        NoteSelector::StableIds {
            note_ids: vec!["note-chorus-root".into()],
        },
    );

    // When: a human transposes that selection and the state is saved/reopened.
    let changed = success(transform(
        &project,
        selection,
        NoteTransform::Transpose { semitones: 12.0 },
    ));
    let reopened: Project = success(serde_json::from_str(&success(serde_json::to_string(
        &changed,
    ))));

    // Then: only the selected source identity changes in every linked instance.
    let notes = success(snapshot(&reopened, None)).notes;
    let chorus = notes
        .iter()
        .filter(|note| note.origin.definition_id.as_str() == "def-chorus")
        .collect::<Vec<_>>();
    assert_eq!(
        chorus
            .iter()
            .filter(|note| note.origin.note_id.as_str() == "note-chorus-root" && note.pitch == 73.0)
            .count(),
        3
    );
    assert!(
        chorus
            .iter()
            .filter(|note| note.origin.note_id.as_str() != "note-chorus-root")
            .all(|note| note.pitch != 73.0)
    );
}

#[test]
fn local_transform_preserves_unaffected_peer_instance() {
    // Given: one note selected in instance-local scope.
    let project = create_project(Some("Local target"), true);
    let target = project.instances[0].id.clone();
    let peer = project.instances[1].id.clone();
    let selection = query(
        &project,
        EditScope::Instance,
        NoteSelector::StableIds {
            note_ids: vec!["note-chorus-root".into()],
        },
    );

    // When: a human applies an instance-local velocity transform.
    let changed = success(transform(
        &project,
        selection,
        NoteTransform::SetVelocity { velocity: 0.25 },
    ));

    // Then: only the selected target instance note changes.
    let notes = success(snapshot(&changed, None)).notes;
    assert_eq!(
        notes
            .iter()
            .find(|note| note.instance_id == target
                && note.origin.note_id.as_str() == "note-chorus-root")
            .map(|note| note.velocity),
        Some(0.25)
    );
    assert_eq!(
        notes
            .iter()
            .find(|note| note.instance_id == peer
                && note.origin.note_id.as_str() == "note-chorus-root")
            .map(|note| note.velocity),
        Some(0.86)
    );
}

#[test]
fn stale_or_tampered_selection_handles_are_rejected() {
    // Given: a valid revision-bound selection handle.
    let project = create_project(Some("Handle checks"), true);
    let selection = query(
        &project,
        EditScope::Source,
        NoteSelector::StableIds {
            note_ids: vec!["note-chorus-root".into()],
        },
    );

    // When: its proof, cardinality, targets, or base project revision is changed.
    let mut tampered = selection.clone();
    tampered.resolution_fingerprint = "forged".into();
    let mut wrong_count = selection.clone();
    wrong_count.cardinality = 2;
    let mut missing = selection.clone();
    missing.note_ids = vec!["missing-note".into()];
    let renamed = success(apply_change(
        &project,
        Principal::Human,
        ChangeRequest {
            expected_revision: project.revision.clone(),
            command: Command::RenameProject {
                name: "Later".into(),
            },
        },
    ));

    // Then: no selected-note mutation is accepted.
    assert_eq!(
        failure(transform(
            &project,
            tampered,
            NoteTransform::Transpose { semitones: 1.0 }
        ))
        .code(),
        "SelectionInvalid"
    );
    assert_eq!(
        failure(transform(
            &project,
            wrong_count,
            NoteTransform::Transpose { semitones: 1.0 }
        ))
        .code(),
        "SelectionInvalid"
    );
    assert_eq!(
        failure(transform(
            &project,
            missing,
            NoteTransform::Transpose { semitones: 1.0 }
        ))
        .code(),
        "SelectionInvalid"
    );
    assert_eq!(
        failure(transform(
            &renamed,
            selection,
            NoteTransform::Transpose { semitones: 1.0 }
        ))
        .code(),
        "SelectionStale"
    );
}
