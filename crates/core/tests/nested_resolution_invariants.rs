use daiw_core::{
    ChangeRequest, ChildPlacement, Command, EditScope, Modifier, Note, NoteSelector, Operation,
    Principal, Project, QueryNotesParams, Request, Response, Tick, apply_change, create_project,
    dispatch, query_notes, snapshot,
};
use serde_json::json;

fn beat(value: f64) -> Tick {
    Tick::from_beats(value).unwrap_or_else(|error| panic!("invalid test beat: {error}"))
}

fn note(id: &str, start: f64, duration: f64) -> Note {
    Note {
        id: id.into(),
        start: beat(start),
        duration: beat(duration),
        pitch: 60.0,
        velocity: 0.8,
    }
}

fn nested_project() -> Project {
    let mut project = create_project(Some("Resolution invariants"), true);
    project.definitions.truncate(2);
    project.instances.truncate(1);
    project.definitions[0].length = beat(4.0);
    project.definitions[0].slots.clear();
    project.definitions[0].notes = vec![note("root", 0.0, 0.5)];
    project.definitions[1].length = beat(1.0);
    project.definitions[1].notes = vec![note("leaf", 0.0, 1.0)];
    project.definitions[0].children = vec![ChildPlacement {
        id: "placement".into(),
        definition_id: project.definitions[1].id.clone(),
        start: beat(1.0),
        length: beat(2.0),
    }];
    project.instances[0].definition_id = project.definitions[0].id.clone();
    project.instances[0].length = beat(4.0);
    project
}

fn explain(project: Project) -> serde_json::Value {
    let instance_id = project.instances[0].id.clone();
    match dispatch(Request {
        id: "explain".into(),
        principal: Principal::Human,
        state: Some(project),
        method: "project.explain".into(),
        params: json!({"instanceId": instance_id}),
    }) {
        Response::Success(response) => response.result,
        Response::Failure(response) => panic!("explain failed: {:?}", response.error),
    }
}

#[test]
fn nested_root_effective_ids_are_hashed_for_own_and_child_notes_while_flat_ids_stay_legacy() {
    // Given
    let nested = nested_project();
    let flat = create_project(Some("Flat IDs"), true);

    // When
    let nested_notes = snapshot(&nested, None)
        .unwrap_or_else(|error| panic!("nested snapshot failed: {error:?}"))
        .notes;
    let flat_notes = snapshot(&flat, None)
        .unwrap_or_else(|error| panic!("flat snapshot failed: {error:?}"))
        .notes;

    // Then
    assert!(
        nested_notes
            .iter()
            .filter(|item| item.instance_id == nested.instances[0].id)
            .all(|item| item.id.as_str().starts_with("nested-effective:"))
    );
    let own = nested_notes
        .iter()
        .find(|item| {
            item.instance_id == nested.instances[0].id && item.origin.note_id.as_str() == "root"
        })
        .unwrap_or_else(|| panic!("missing root-owned note"));
    assert!(own.origin.definition_path.is_none());
    let expected = format!(
        "{}@{}#0",
        flat.definitions[0].notes[0].id, flat.instances[0].id
    );
    assert!(flat_notes.iter().any(|item| item.id.as_str() == expected));
}

#[test]
fn editable_notes_crop_to_instance_first_loop_and_keep_nested_pattern_ids() {
    // Given
    let mut project = nested_project();
    project.instances[0].length = beat(1.5);

    // When
    let result = explain(project.clone());
    let selection = query_notes(
        &project,
        QueryNotesParams {
            instance_id: project.instances[0].id.clone(),
            scope: EditScope::Instance,
            selector: NoteSelector::All,
            max_results: 16,
        },
    )
    .unwrap_or_else(|error| panic!("selection failed: {error:?}"));

    // Then
    assert_eq!(result["compositionNotes"].as_array().map(Vec::len), Some(3));
    assert_eq!(result["effectiveNotes"].as_array().map(Vec::len), Some(2));
    assert_eq!(result["effectiveNotes"][1]["duration"], json!(0.5));
    assert!(
        result["effectiveNotes"][1]["id"]
            .as_str()
            .is_some_and(|id| id.starts_with("nested:"))
    );
    assert_eq!(selection.cardinality, 2);
}

#[test]
fn nested_composition_sort_makes_binding_targets_independent_of_child_array_order() {
    // Given
    let mut first = nested_project();
    let leaf_id = first.definitions[1].id.clone();
    first.definitions[0].children.push(ChildPlacement {
        id: "earlier".into(),
        definition_id: leaf_id,
        start: beat(0.5),
        length: beat(1.0),
    });
    let mut second = first.clone();
    second.definitions[0].children.reverse();
    for project in [&mut first, &mut second] {
        project.modifiers.push(Modifier {
            id: "replace".into(),
            name: "Replace".into(),
            operations: vec![Operation::ReplaceSlot {
                slot: "notes".into(),
                notes: vec![note("replacement", 0.0, 0.5)],
            }],
        });
    }

    // When
    let attach = |project: &Project| {
        apply_change(
            project,
            Principal::Human,
            ChangeRequest {
                expected_revision: project.revision.clone(),
                command: Command::ApplyModifier {
                    instance_id: project.instances[0].id.clone(),
                    modifier_id: "replace".into(),
                    ordering: None,
                },
            },
        )
        .unwrap_or_else(|error| panic!("attachment failed: {error:?}"))
    };
    let first = attach(&first);
    let second = attach(&second);

    // Then
    assert_eq!(
        first.instances[0].modifier_bindings[&daiw_core::ModifierId::from("replace")].slots["notes"].note_ids,
        second.instances[0].modifier_bindings[&daiw_core::ModifierId::from("replace")].slots["notes"].note_ids,
    );
}
