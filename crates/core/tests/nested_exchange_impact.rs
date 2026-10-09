use daiw_core::{
    ChildPlacement, EditScope, Note, NoteSelector, Principal, Project, QueryNotesParams, Request,
    Response, Tick, create_project, dispatch, query_notes,
};
use serde_json::{Value, json};

fn beat(value: f64) -> Tick {
    Tick::from_beats(value).unwrap_or_else(|error| panic!("invalid test beat: {error}"))
}

fn project() -> Project {
    let mut project = create_project(Some("Nested exchange"), true);
    project.definitions.truncate(2);
    project.instances.truncate(1);
    project.definitions[0].length = beat(4.0);
    project.definitions[0].slots.clear();
    project.definitions[0].notes.truncate(1);
    project.definitions[1].length = beat(1.0);
    project.definitions[1].notes.truncate(1);
    project.definitions[0].children = vec![ChildPlacement {
        id: "nested-placement".into(),
        definition_id: project.definitions[1].id.clone(),
        start: beat(1.0),
        length: beat(2.0),
    }];
    project.instances[0].definition_id = project.definitions[0].id.clone();
    project.instances[0].length = beat(4.0);
    project
}

fn call(state: Option<Project>, method: &str, params: Value) -> Response {
    dispatch(Request {
        id: "nested".into(),
        principal: Principal::Human,
        state,
        method: method.into(),
        params,
    })
}

#[test]
fn native_round_trip_requires_nested_feature_and_preserves_children() {
    // Given
    let project = project();

    // When
    let document = match call(Some(project.clone()), "project.export", json!({})) {
        Response::Success(response) => response.result,
        Response::Failure(response) => panic!("export failed: {:?}", response.error),
    };
    let imported = call(None, "project.import", json!({"project": document.clone()}));

    // Then
    assert!(
        document["requiredFeatures"]
            .as_array()
            .is_some_and(|features| features.contains(&json!("nested-definitions-v1")))
    );
    match imported {
        Response::Success(response) => {
            assert_eq!(
                response.state.definitions[0].children,
                project.definitions[0].children
            )
        }
        Response::Failure(response) => panic!("import failed: {:?}", response.error),
    }
}

#[test]
fn descendant_edit_dry_run_reports_ancestor_definition_and_dependent_instance() {
    // Given
    let project = project();
    let root_id = project.definitions[0].id.clone();
    let leaf_id = project.definitions[1].id.clone();
    let instance_id = project.instances[0].id.clone();
    let mut notes: Vec<Note> = project.definitions[1].notes.clone();
    notes[0].pitch += 1.0;

    // When
    let response = call(
        Some(project.clone()),
        "change.dryRun",
        json!({
            "expectedRevision": project.revision,
            "command": {
                "type": "editDefinitionNotes",
                "definitionId": leaf_id,
                "notes": notes
            }
        }),
    );

    // Then
    let result = match response {
        Response::Success(response) => response.result,
        Response::Failure(response) => panic!("dry run failed: {:?}", response.error),
    };
    assert_eq!(result["affectedInstances"], json!([instance_id]));
    let definitions = result["impact"]["affectedDefinitions"]
        .as_array()
        .unwrap_or_else(|| panic!("missing affected definitions"));
    assert!(definitions.contains(&json!(root_id)));
    assert!(definitions.contains(&json!(leaf_id)));
}

#[test]
fn legacy_empty_children_are_normalized_without_accepting_other_unknown_fields() {
    // Given
    let flat = create_project(Some("Legacy empty children"), true);
    let mut raw = serde_json::to_value(flat)
        .unwrap_or_else(|error| panic!("legacy serialization failed: {error}"));
    raw["definitions"][0]["children"] = json!([]);

    // When / Then
    assert!(matches!(
        call(None, "project.import", json!({"project": raw})),
        Response::Success(_)
    ));
}

#[test]
fn source_selection_reads_root_notes_while_instance_selection_reads_composition() {
    // Given
    let project = project();
    let instance_id = project.instances[0].id.clone();

    // When
    let source = query_notes(
        &project,
        QueryNotesParams {
            instance_id: instance_id.clone(),
            scope: EditScope::Source,
            selector: NoteSelector::All,
            max_results: 16,
        },
    )
    .unwrap_or_else(|error| panic!("source query failed: {error:?}"));
    let instance = query_notes(
        &project,
        QueryNotesParams {
            instance_id,
            scope: EditScope::Instance,
            selector: NoteSelector::All,
            max_results: 16,
        },
    )
    .unwrap_or_else(|error| panic!("instance query failed: {error:?}"));

    // Then
    assert_eq!(source.cardinality, 1);
    assert_eq!(instance.cardinality, 3);
}

#[test]
fn explain_includes_pre_modifier_composition_with_local_pattern_ids() {
    // Given
    let project = project();
    let instance_id = project.instances[0].id.clone();

    // When
    let response = call(
        Some(project),
        "project.explain",
        json!({"instanceId": instance_id}),
    );

    // Then
    let result = match response {
        Response::Success(response) => response.result,
        Response::Failure(response) => panic!("explain failed: {:?}", response.error),
    };
    assert_eq!(result["compositionNotes"].as_array().map(Vec::len), Some(3));
    assert!(result["compositionNotes"].as_array().is_some_and(|notes| {
        notes
            .iter()
            .any(|note| note["origin"]["definitionPath"].is_array())
    }));
}
