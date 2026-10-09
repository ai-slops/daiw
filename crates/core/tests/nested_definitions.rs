use daiw_core::{
    ChangeRequest, ChildPlacement, Command, DefinitionRevision, Note, Principal, Tick,
    apply_change, create_project, snapshot, validate_project,
};
use serde_json::json;
use std::collections::BTreeSet;

fn beat(value: f64) -> Tick {
    Tick::from_beats(value).unwrap_or_else(|error| panic!("invalid test beat: {error}"))
}

fn note(id: &str, start: f64, duration: f64, pitch: f64) -> Note {
    Note {
        id: id.into(),
        start: beat(start),
        duration: beat(duration),
        pitch,
        velocity: 0.8,
    }
}

fn nested_project() -> daiw_core::Project {
    let mut project = create_project(Some("Nested"), true);
    project.definitions.truncate(3);
    project.instances.truncate(1);
    project.instances[0].definition_id = project.definitions[0].id.clone();
    project.instances[0].length = beat(8.0);
    project.definitions[0].length = beat(4.0);
    project.definitions[0].notes = vec![note("root", 0.0, 0.5, 60.0)];
    project.definitions[0].slots.clear();
    project.definitions[1].length = beat(2.0);
    project.definitions[1].notes = vec![note("child", 0.0, 0.5, 64.0)];
    project.definitions[2].length = beat(2.0);
    project.definitions[2].notes = vec![note("leaf", 0.75, 0.5, 67.0)];
    project.definitions[1].children = vec![ChildPlacement {
        id: "grandchild-placement".into(),
        definition_id: project.definitions[2].id.clone(),
        start: beat(0.5),
        length: beat(1.0),
    }];
    project.definitions[0].children = vec![ChildPlacement {
        id: "child-placement".into(),
        definition_id: project.definitions[1].id.clone(),
        start: beat(1.0),
        length: beat(3.0),
    }];
    project
}

#[test]
fn nested_composition_loops_crops_and_inherits_the_instance_track() {
    // Given
    let project = nested_project();

    // When
    let rendered = snapshot(&project, None)
        .unwrap_or_else(|error| panic!("nested snapshot failed: {error:?}"));
    let notes: Vec<_> = rendered
        .notes
        .iter()
        .filter(|note| note.instance_id == project.instances[0].id)
        .collect();

    // Then
    assert_eq!(notes.len(), 8);
    assert!(
        notes
            .iter()
            .all(|note| note.track_id == project.instances[0].track_id)
    );
    assert!(notes.iter().all(|note| {
        note.start + note.duration <= project.instances[0].start + project.instances[0].length
    }));
    assert!(notes.iter().any(|note| note.duration == beat(0.25)));
    let nested = notes
        .iter()
        .find(|note| note.origin.note_id.as_str() == "leaf")
        .unwrap_or_else(|| panic!("missing nested leaf"));
    assert_eq!(nested.origin.instance_loop_index, Some(0));
    assert_eq!(
        nested.origin.definition_path.as_ref().map(Vec::len),
        Some(3)
    );
}

#[test]
fn nested_pattern_ids_ignore_timing_order_and_revisions_but_repeats_are_unique() {
    // Given
    let project = nested_project();
    let first =
        snapshot(&project, None).unwrap_or_else(|error| panic!("first snapshot failed: {error:?}"));
    let mut changed = project.clone();
    changed.definitions[0].children[0].start = beat(0.5);
    changed.definitions[0].children.reverse();
    changed.definitions[0].revision += 1;
    changed.definitions[1].revision += 1;

    // When
    let second = snapshot(&changed, None)
        .unwrap_or_else(|error| panic!("second snapshot failed: {error:?}"));
    let nested_ids = |notes: &[daiw_core::EffectiveNote]| {
        notes
            .iter()
            .filter(|note| note.origin.definition_path.is_some())
            .map(|note| note.id.clone())
            .collect::<BTreeSet<_>>()
    };

    // Then
    assert_eq!(nested_ids(&first.notes), nested_ids(&second.notes));
    assert_eq!(nested_ids(&first.notes).len(), 6);
}

#[test]
fn validation_rejects_cycles_duplicate_placements_and_out_of_range_children() {
    // Given
    let mut cycle = nested_project();
    let cycle_target = cycle.definitions[0].id.clone();
    cycle.definitions[2].children.push(ChildPlacement {
        id: "cycle".into(),
        definition_id: cycle_target,
        start: beat(0.0),
        length: beat(1.0),
    });
    let mut duplicate = nested_project();
    let duplicate_target = duplicate.definitions[1].id.clone();
    duplicate.definitions[2].children.push(ChildPlacement {
        id: "child-placement".into(),
        definition_id: duplicate_target,
        start: beat(0.0),
        length: beat(1.0),
    });
    let mut range = nested_project();
    range.definitions[0].children[0].length = beat(4.0);

    // When / Then
    for project in [&cycle, &duplicate, &range] {
        assert!(validate_project(project).is_err());
    }
}

#[test]
fn child_commands_use_exact_wire_shape_increment_only_parent_and_reject_noop() {
    // Given
    let project = nested_project();
    let parent = project.definitions[0].id.clone();
    let child = project.definitions[1].id.clone();
    let command: Command = serde_json::from_value(json!({
        "type": "addDefinitionChild",
        "parentDefinitionId": parent,
        "definitionId": child,
        "start": 0,
        "length": 2.0
    }))
    .unwrap_or_else(|error| panic!("wire command failed: {error}"));

    // When
    let changed = apply_change(
        &project,
        Principal::Human,
        ChangeRequest {
            expected_revision: project.revision.clone(),
            command,
        },
    )
    .unwrap_or_else(|error| panic!("add child failed: {error:?}"));
    let added = changed.definitions[0]
        .children
        .last()
        .cloned()
        .unwrap_or_else(|| panic!("missing child"));
    let no_op = apply_change(
        &changed,
        Principal::Human,
        ChangeRequest {
            expected_revision: changed.revision.clone(),
            command: Command::SetDefinitionChild {
                parent_definition_id: changed.definitions[0].id.clone(),
                child: added,
            },
        },
    );

    // Then
    assert_eq!(
        changed.definitions[0].revision,
        project.definitions[0].revision + 1
    );
    assert_eq!(
        changed.definitions[1].revision,
        project.definitions[1].revision
    );
    assert!(no_op.is_err());
}

#[test]
fn nested_revision_metadata_is_sorted_and_omitted_for_flat_bindings() {
    // Given
    let project = nested_project();

    // When
    let revisions = daiw_core::definition_revisions(&project, &project.definitions[0].id)
        .unwrap_or_else(|error| panic!("revision closure failed: {error:?}"));
    let flat = daiw_core::definition_revisions(&project, &project.definitions[2].id)
        .unwrap_or_else(|error| panic!("flat closure failed: {error:?}"));

    // Then
    assert_eq!(revisions.len(), 3);
    assert!(
        revisions
            .windows(2)
            .all(|pair| pair[0].definition_id < pair[1].definition_id)
    );
    assert_eq!(
        flat,
        vec![DefinitionRevision {
            definition_id: project.definitions[2].id.clone(),
            revision: project.definitions[2].revision,
        }]
    );
}
