use daiw_core::{
    ChangeRequest, Command, CoreError, EditScope, Note, NotePatch, Operation, Principal, Project,
    ProposalParams, Request, Response, Tick, apply_change, create_project, create_proposal,
    dispatch, snapshot, validate_project,
};

fn edit(project: &Project, scope: EditScope, notes: Vec<Note>) -> Result<Project, CoreError> {
    apply_change(
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
    )
}

#[test]
fn cancelled_local_addition_does_not_conflict_with_future_source_addition() -> Result<(), CoreError>
{
    let project = create_project(None, true);
    let original = project.definitions[0].notes.clone();
    let mut local = original.clone();
    let mut added = original[0].clone();
    added.id = "cancelled-addition".into();
    local.push(added.clone());
    let project = edit(&project, EditScope::Instance, local)?;
    let project = edit(&project, EditScope::Instance, original.clone())?;
    let mut source = original;
    source.push(added);

    let changed = edit(&project, EditScope::Source, source)?;

    assert!(changed.instances[0].overrides.is_empty());
    snapshot(&changed, None)?;
    Ok(())
}

#[test]
fn restored_local_note_follows_future_source_edit() -> Result<(), CoreError> {
    let project = create_project(None, true);
    let original = project.definitions[0].notes.clone();
    let project = edit(&project, EditScope::Instance, original[1..].to_vec())?;
    let project = edit(&project, EditScope::Instance, original.clone())?;
    let mut source = original;
    source[0].pitch = 90.0;

    let changed = edit(&project, EditScope::Source, source)?;

    let rendered = snapshot(&changed, None)?;
    assert!(changed.instances[0].overrides.is_empty());
    assert!(rendered.notes.iter().any(|note| {
        note.instance_id == changed.instances[0].id
            && note.origin.note_id.as_str() == "note-chorus-root"
            && note.pitch == 90.0
    }));
    Ok(())
}

#[test]
fn unresolved_neighbor_does_not_block_explaining_or_editing_healthy_instance()
-> Result<(), CoreError> {
    let project = create_project(None, true);
    let mut local = project.definitions[0].notes.clone();
    local[0].pitch = 49.0;
    let project = edit(&project, EditScope::Instance, local)?;
    let mut project = edit(
        &project,
        EditScope::Source,
        project.definitions[0].notes[1..].to_vec(),
    )?;
    project.instances.swap(0, 1);

    let changed = edit(
        &project,
        EditScope::Instance,
        project.definitions[0].notes.clone(),
    )?;

    let id = changed.instances[0].id.clone();
    let response = dispatch(Request {
        id: "explain-healthy".into(),
        principal: Principal::Human,
        state: Some(changed),
        method: "project.explain".into(),
        params: serde_json::json!({"instanceId": id}),
    });
    assert!(matches!(response, Response::Success(_)));
    Ok(())
}

#[test]
fn local_notes_must_fit_the_editable_first_loop() -> Result<(), Box<dyn std::error::Error>> {
    for (length, start, duration) in [(16.0, 8.0, 1.0), (16.0, 7.0, 2.0), (4.0, 4.0, 1.0)] {
        let mut project = create_project(None, true);
        project.instances[0].length = Tick::from_beats(length)?;
        let mut note = project.definitions[0].notes[0].clone();
        note.start = Tick::from_beats(start)?;
        note.duration = Tick::from_beats(duration)?;

        let result = edit(&project, EditScope::Instance, vec![note]);

        assert!(matches!(result, Err(CoreError::InvalidProject(_))));
    }
    Ok(())
}

#[test]
fn imported_patch_notes_must_fit_the_definition() -> Result<(), Box<dyn std::error::Error>> {
    let mut project = create_project(None, true);
    let mut note = project.definitions[0].notes[0].clone();
    note.start = Tick::from_beats(8.0)?;
    project.instances[0]
        .overrides
        .push(Operation::PatchNotes(NotePatch {
            updates: vec![note],
            added: vec![],
            removed: vec![],
        }));

    let result = validate_project(&project);

    assert!(matches!(result, Err(CoreError::InvalidProject(_))));
    Ok(())
}

#[test]
fn imported_proposals_cannot_carry_internal_note_patches() -> Result<(), CoreError> {
    let mut project = create_project(None, true);
    let mut proposal = create_proposal(
        &project,
        ProposalParams {
            expected_revision: project.revision.clone(),
            instance_id: project.instances[0].id.clone(),
            request: "octave up".into(),
            operations: None,
        },
    )?;
    proposal.hunks[0].operation = Operation::PatchNotes(NotePatch {
        updates: vec![],
        added: vec![],
        removed: vec![],
    });
    project.proposals.push(proposal);

    let result = validate_project(&project);

    assert!(matches!(result, Err(CoreError::UnsupportedCapability(_))));
    Ok(())
}
