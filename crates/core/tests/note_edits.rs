use daiw_core::{
    ChangeRequest, Command, EditScope, Note, Principal, Project, apply_change, create_project,
    snapshot,
};

fn success<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("unexpected error: {error:?}"),
    }
}

fn present<T>(value: Option<T>) -> T {
    match value {
        Some(value) => value,
        None => panic!("expected value"),
    }
}

fn failure<T, E>(result: Result<T, E>) -> E {
    match result {
        Ok(_) => panic!("unexpected success"),
        Err(error) => error,
    }
}

fn edit(project: &Project, scope: EditScope, notes: Vec<Note>) -> Project {
    success(apply_change(
        project,
        Principal::Human,
        ChangeRequest {
            expected_revision: project.revision.clone(),
            command: Command::EditNotes {
                instance_id: project.instances[0].id.clone(),
                scope,
                notes,
            },
        },
    ))
}

#[test]
fn unrelated_source_changes_propagate_after_local_note_edit() {
    // Given: one locally changed note in a linked instance.
    let project = create_project(Some("Note identity"), true);
    let mut local = project.definitions[0].notes.clone();
    local[0].pitch = 49.0;
    let project = edit(&project, EditScope::Instance, local);
    let mut source = project.definitions[0].notes.clone();
    source[1].pitch = 77.0;
    let mut added = source[2].clone();
    added.id = "new-source-note".into();
    source.push(added);

    // When: the shared source changes unrelated notes.
    let changed = edit(&project, EditScope::Source, source);

    // Then: the local change coexists with source updates and additions.
    let rendered = success(snapshot(&changed, None));
    let notes: Vec<_> = rendered
        .notes
        .iter()
        .filter(|note| note.instance_id == changed.instances[0].id)
        .collect();
    assert_eq!(notes.len(), 5);
    assert_eq!(
        present(
            notes
                .iter()
                .find(|note| note.origin.note_id.as_str() == "note-chorus-root")
        )
        .pitch,
        49.0
    );
    assert_eq!(
        present(
            notes
                .iter()
                .find(|note| note.origin.note_id.as_str() == "note-chorus-two")
        )
        .pitch,
        77.0
    );
    assert!(
        notes
            .iter()
            .any(|note| note.origin.note_id.as_str() == "new-source-note")
    );
    assert_eq!(changed.definitions[0].notes[0].pitch, 61.0);
}

#[test]
fn source_deletion_of_locally_edited_target_is_explicit() {
    // Given: a local edit bound to a source note ID.
    let project = create_project(Some("Missing target"), true);
    let mut local = project.definitions[0].notes.clone();
    local[0].pitch = 49.0;
    let project = edit(&project, EditScope::Instance, local);
    let source = project.definitions[0].notes[1..].to_vec();

    // When: that target is removed from the shared definition.
    let changed = edit(&project, EditScope::Source, source);

    // Then: rendering requires rebase instead of resurrecting the deleted note.
    assert_eq!(failure(snapshot(&changed, None)).code(), "RebaseConflict");
}

#[test]
fn local_add_remove_and_repeated_edits_survive_round_trip() {
    let project = create_project(Some("Repeated local edits"), true);
    let mut local = project.definitions[0].notes.clone();
    local.remove(0);
    let mut added = local[0].clone();
    added.id = "local-added".into();
    added.pitch = 82.0;
    local.push(added);
    let project = edit(&project, EditScope::Instance, local.clone());
    present(local.last_mut()).pitch = 84.0;
    let project = edit(&project, EditScope::Instance, local);
    let mut source = project.definitions[0].notes.clone();
    source[2].velocity = 0.42;
    source.remove(3);
    let project = edit(&project, EditScope::Source, source);

    let restored: Project = success(serde_json::from_str(&success(serde_json::to_string(
        &project,
    ))));

    let rendered = success(snapshot(&restored, None));
    let notes: Vec<_> = rendered
        .notes
        .iter()
        .filter(|note| note.instance_id == restored.instances[0].id)
        .collect();
    assert_eq!(notes.len(), 3);
    assert!(
        notes
            .iter()
            .all(|note| note.origin.note_id.as_str() != "note-chorus-root")
    );
    assert_eq!(
        present(
            notes
                .iter()
                .find(|note| note.origin.note_id.as_str() == "local-added")
        )
        .pitch,
        84.0
    );
    assert_eq!(
        present(
            notes
                .iter()
                .find(|note| note.origin.note_id.as_str() == "note-chorus-three")
        )
        .velocity,
        0.42
    );
}

#[test]
fn unchanged_modifier_notes_are_not_baked_into_local_patch() {
    let mut project = create_project(Some("Modifier context"), true);
    project.modifiers.push(daiw_core::Modifier {
        id: "transpose".into(),
        name: "Octave".into(),
        operations: vec![daiw_core::Operation::Transpose { semitones: 12.0 }],
    });
    project.instances[0].modifiers.push("transpose".into());
    let mut local = project.definitions[0].notes.clone();
    for note in &mut local {
        note.pitch += 12.0;
    }
    local[0].pitch = 50.0;
    let project = edit(&project, EditScope::Instance, local);
    let mut source = project.definitions[0].notes.clone();
    source[1].pitch = 70.0;

    let changed = edit(&project, EditScope::Source, source);

    let rendered = success(snapshot(&changed, None));
    let note = present(rendered.notes.iter().find(|note| {
        note.instance_id == changed.instances[0].id
            && note.origin.note_id.as_str() == "note-chorus-two"
    }));
    assert_eq!(note.pitch, 82.0);
    assert_eq!(
        note.origin.modifier_ids,
        vec![daiw_core::ModifierId::from("transpose")]
    );
}

#[test]
fn source_collision_with_local_addition_requires_rebase() {
    let project = create_project(Some("Collision"), true);
    let mut local = project.definitions[0].notes.clone();
    let mut addition = local[0].clone();
    addition.id = "shared-collision".into();
    local.push(addition.clone());
    let project = edit(&project, EditScope::Instance, local);
    let mut source = project.definitions[0].notes.clone();
    source.push(addition);

    let changed = edit(&project, EditScope::Source, source);

    assert_eq!(failure(snapshot(&changed, None)).code(), "RebaseConflict");
}

#[test]
fn unchanged_notes_do_not_create_local_overrides() {
    let project = create_project(Some("No-op"), true);
    let notes = project.definitions[0].notes.clone();

    let changed = edit(&project, EditScope::Instance, notes);

    assert!(changed.instances[0].overrides.is_empty());
}
