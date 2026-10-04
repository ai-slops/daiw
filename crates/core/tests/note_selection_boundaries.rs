use daiw_core::{
    ChangeRequest, Command, CoreError, EditScope, NoteSelector, NoteTransform, Principal, Project,
    QueryNotesParams, SelectionHandle, Tick, apply_change, create_project, query_notes,
};

fn query(project: &Project, selector: NoteSelector) -> Result<SelectionHandle, CoreError> {
    query_notes(
        project,
        QueryNotesParams {
            instance_id: project.instances[0].id.clone(),
            scope: EditScope::Source,
            selector,
            max_results: 16,
        },
    )
}

#[test]
fn agent_may_preview_but_cannot_apply_selected_transform() -> Result<(), CoreError> {
    let project = create_project(Some("Authorization"), true);
    let command = Command::TransformSelectedNotes {
        selection: query(&project, NoteSelector::All)?,
        transform: NoteTransform::Transpose { semitones: 1.0 },
    };
    let preview = daiw_core::preview_change(&project, &project.revision, command.clone());
    let applied = apply_change(
        &project,
        Principal::Agent,
        ChangeRequest {
            expected_revision: project.revision.clone(),
            command,
        },
    );
    assert!(preview.is_ok());
    assert!(matches!(applied, Err(CoreError::PermissionDenied)));
    Ok(())
}

#[test]
fn missing_slot_is_an_error_and_virtual_notes_selects_whole_pattern() -> Result<(), CoreError> {
    let project = create_project(None, true);
    assert!(matches!(
        query(
            &project,
            NoteSelector::NamedSlot {
                slot: "missing".into()
            }
        ),
        Err(CoreError::SelectionInvalid(_))
    ));
    let all = query(
        &project,
        NoteSelector::NamedSlot {
            slot: "notes".into(),
        },
    )?;
    assert_eq!(all.cardinality, 4);
    Ok(())
}

#[test]
fn beat_ranges_follow_existing_half_open_onset_semantics() -> Result<(), CoreError> {
    let project = create_project(None, true);
    let selected = query(
        &project,
        NoteSelector::BeatRange {
            start: Tick::default(),
            end: Tick::from_ticks(960),
        },
    )?;
    assert_eq!(
        selected.note_ids,
        vec![daiw_core::NoteId::from("note-chorus-root")]
    );
    Ok(())
}

#[test]
fn pitch_selection_preserves_finite_non_midi_values_and_signed_zero() -> Result<(), CoreError> {
    let mut project = create_project(None, true);
    project.definitions[0].notes[0].pitch = 140.25;
    let selected = query(
        &project,
        NoteSelector::PitchRange {
            min: 140.0,
            max: 141.0,
        },
    )?;
    assert_eq!(selected.cardinality, 1);
    let zero = query(
        &project,
        NoteSelector::PitchRange {
            min: -0.0,
            max: 0.0,
        },
    )?;
    let positive = query(&project, NoteSelector::PitchRange { min: 0.0, max: 0.0 })?;
    assert_eq!(zero.resolution_fingerprint, positive.resolution_fingerprint);
    Ok(())
}

#[test]
fn selection_is_project_bound_even_when_revisions_are_equal() -> Result<(), CoreError> {
    let project = create_project(None, true);
    let selection = query(&project, NoteSelector::All)?;
    let mut other = project.clone();
    other.id = "different-project".into();
    let result = apply_change(
        &other,
        Principal::Human,
        ChangeRequest {
            expected_revision: other.revision.clone(),
            command: Command::TransformSelectedNotes {
                selection,
                transform: NoteTransform::Transpose { semitones: 12.0 },
            },
        },
    );
    assert!(matches!(result, Err(CoreError::SelectionStale)));
    Ok(())
}

#[test]
fn empty_and_unchanged_transforms_cannot_advance_source_revision() -> Result<(), CoreError> {
    let project = create_project(None, true);
    for (selector, transform) in [
        (
            NoteSelector::PitchRange {
                min: 999.0,
                max: 1000.0,
            },
            NoteTransform::Transpose { semitones: 12.0 },
        ),
        (
            NoteSelector::All,
            NoteTransform::Transpose { semitones: 0.0 },
        ),
    ] {
        let selection = query(&project, selector)?;
        let result = apply_change(
            &project,
            Principal::Human,
            ChangeRequest {
                expected_revision: project.revision.clone(),
                command: Command::TransformSelectedNotes {
                    selection,
                    transform,
                },
            },
        );
        assert!(matches!(result, Err(CoreError::InvalidParams(_))));
    }
    Ok(())
}
