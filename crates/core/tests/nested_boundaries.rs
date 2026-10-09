use daiw_core::{
    ChangeRequest, ChildPlacement, Command, Definition, EditScope, Note, NoteSelector, Principal,
    QueryNotesParams, Tick, apply_change, create_project, query_notes, snapshot, validate_project,
};
use std::collections::BTreeMap;

fn definition(id: &str, length: Tick, notes: Vec<Note>) -> Definition {
    Definition {
        id: id.into(),
        name: id.into(),
        length,
        notes,
        revision: 0,
        slots: BTreeMap::new(),
        children: Vec::new(),
    }
}

fn note(id: &str) -> Note {
    Note {
        id: id.into(),
        start: Tick::from_ticks(0),
        duration: Tick::from_ticks(1),
        pitch: 60.0,
        velocity: 0.8,
    }
}

#[test]
fn validation_enforces_sixteen_placement_steps_and_global_placement_limit() {
    // Given
    let mut deep = create_project(Some("Deep"), true);
    deep.definitions = (0..18)
        .map(|index| definition(&format!("d{index}"), Tick::from_ticks(1), Vec::new()))
        .collect();
    for index in 0..17 {
        let child_id = deep.definitions[index + 1].id.clone();
        deep.definitions[index].children.push(ChildPlacement {
            id: format!("p{index}").into(),
            definition_id: child_id,
            start: Tick::from_ticks(0),
            length: Tick::from_ticks(1),
        });
    }
    deep.instances.truncate(1);
    deep.instances[0].definition_id = deep.definitions[0].id.clone();
    deep.instances[0].length = Tick::from_ticks(1);
    let mut crowded = create_project(Some("Crowded"), true);
    crowded.definitions.truncate(2);
    crowded.definitions[0].children = (0..4_097)
        .map(|index| ChildPlacement {
            id: format!("placement-{index}").into(),
            definition_id: crowded.definitions[1].id.clone(),
            start: Tick::from_ticks(0),
            length: Tick::from_ticks(1),
        })
        .collect();

    // When / Then
    assert!(validate_project(&deep).is_err());
    assert!(validate_project(&crowded).is_err());
}

#[test]
fn empty_child_loops_consume_composition_work_before_output_allocation() {
    // Given
    let mut project = create_project(Some("Empty loop budget"), true);
    project.definitions = vec![
        definition("root", Tick::from_ticks(100_001), Vec::new()),
        definition("empty", Tick::from_ticks(1), Vec::new()),
    ];
    project.definitions[0].children.push(ChildPlacement {
        id: "empty-loop".into(),
        definition_id: "empty".into(),
        start: Tick::from_ticks(0),
        length: Tick::from_ticks(100_001),
    });
    project.instances.truncate(1);
    project.instances[0].definition_id = "root".into();
    project.instances[0].length = Tick::from_ticks(100_001);

    // When
    let result = snapshot(&project, None);

    // Then
    assert!(result.is_err());
}

#[test]
fn hostile_source_id_colliding_with_nested_hash_is_rejected() {
    // Given
    let mut project = create_project(Some("Hostile IDs"), true);
    project.definitions = vec![
        definition("root", Tick::from_ticks(2), vec![note("root-note")]),
        definition("leaf", Tick::from_ticks(1), vec![note("leaf-note")]),
    ];
    project.definitions[0].children.push(ChildPlacement {
        id: "placement".into(),
        definition_id: "leaf".into(),
        start: Tick::from_ticks(1),
        length: Tick::from_ticks(1),
    });
    project.instances.truncate(1);
    project.instances[0].definition_id = "root".into();
    project.instances[0].length = Tick::from_ticks(2);
    let selection = query_notes(
        &project,
        QueryNotesParams {
            instance_id: project.instances[0].id.clone(),
            scope: EditScope::Instance,
            selector: NoteSelector::All,
            max_results: 4,
        },
    )
    .unwrap_or_else(|error| panic!("initial selection failed: {error:?}"));
    project.definitions[0].notes[0].id = selection.note_ids[1].clone();

    // When
    let result = snapshot(&project, None);

    // Then
    assert!(result.is_err());
}

#[test]
fn invalid_child_command_does_not_mutate_supplied_project() {
    // Given
    let project = create_project(Some("No mutation"), true);
    let before = project.clone();

    // When
    let result = apply_change(
        &project,
        Principal::Human,
        ChangeRequest {
            expected_revision: project.revision.clone(),
            command: Command::AddDefinitionChild {
                parent_definition_id: project.definitions[0].id.clone(),
                definition_id: project.definitions[1].id.clone(),
                start: Tick::from_ticks(-1),
                length: Tick::from_ticks(1),
            },
        },
    );

    // Then
    assert!(result.is_err());
    assert_eq!(project, before);
}

#[test]
fn layered_shared_graph_is_bounded_and_cached_depth_remains_path_independent() {
    // Given
    let mut layered = create_project(Some("Layered DAG"), true);
    layered.definitions = (0..20)
        .map(|index| definition(&format!("layer-{index}"), Tick::from_ticks(1), Vec::new()))
        .collect();
    for layer in 0..9 {
        for side in 0..2 {
            let index = layer * 2 + side;
            for next_side in 0..2 {
                let child_id = layered.definitions[(layer + 1) * 2 + next_side].id.clone();
                layered.definitions[index].children.push(ChildPlacement {
                    id: format!("edge-{index}-{next_side}").into(),
                    definition_id: child_id,
                    start: Tick::from_ticks(0),
                    length: Tick::from_ticks(1),
                });
            }
        }
    }
    layered.instances.truncate(1);
    layered.instances[0].definition_id = layered.definitions[0].id.clone();
    layered.instances[0].length = Tick::from_ticks(1);
    let mut entrance = create_project(Some("Entrance depth"), true);
    entrance.definitions = (0..19)
        .map(|index| definition(&format!("depth-{index}"), Tick::from_ticks(1), Vec::new()))
        .collect();
    let shared_id = entrance.definitions[17].id.clone();
    entrance.definitions[0].children.push(ChildPlacement {
        id: "shallow".into(),
        definition_id: shared_id.clone(),
        start: Tick::from_ticks(0),
        length: Tick::from_ticks(1),
    });
    for index in 0..16 {
        let child_id = entrance.definitions[index + 1].id.clone();
        entrance.definitions[index].children.push(ChildPlacement {
            id: format!("deep-{index}").into(),
            definition_id: child_id,
            start: Tick::from_ticks(0),
            length: Tick::from_ticks(1),
        });
    }
    entrance.definitions[16].children.push(ChildPlacement {
        id: "deep-shared".into(),
        definition_id: shared_id,
        start: Tick::from_ticks(0),
        length: Tick::from_ticks(1),
    });
    let past_limit_id = entrance.definitions[18].id.clone();
    entrance.definitions[17].children.push(ChildPlacement {
        id: "past-limit".into(),
        definition_id: past_limit_id,
        start: Tick::from_ticks(0),
        length: Tick::from_ticks(1),
    });
    entrance.instances.truncate(1);
    entrance.instances[0].definition_id = entrance.definitions[0].id.clone();
    entrance.instances[0].length = Tick::from_ticks(1);

    // When / Then
    assert!(validate_project(&layered).is_ok());
    assert!(validate_project(&entrance).is_err());
}

#[test]
fn maximum_definition_revision_rejects_new_edits_without_mutation() {
    // Given
    let mut project = create_project(Some("Revision overflow"), true);
    project.definitions[0].revision = u64::MAX;
    let before = project.clone();
    let mut notes = project.definitions[0].notes.clone();
    notes[0].pitch += 1.0;

    // When
    let result = apply_change(
        &project,
        Principal::Human,
        ChangeRequest {
            expected_revision: project.revision.clone(),
            command: Command::EditDefinitionNotes {
                definition_id: project.definitions[0].id.clone(),
                notes,
            },
        },
    );

    // Then
    assert!(result.is_err());
    assert_eq!(project, before);
}
