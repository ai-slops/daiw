use daiw_core::{
    CoreError, Modifier, ModifierOrdering, Operation, Principal, Project, Request, Response, Tick,
    create_project, dispatch, snapshot, validate_project,
};
use serde_json::json;

fn invalid(project: &Project) {
    assert!(matches!(
        snapshot(project, None),
        Err(CoreError::InvalidProject(_))
    ));
}

#[test]
fn added_source_id_collision_is_rejected_in_both_ordering_modes() -> Result<(), CoreError> {
    for ordering in [None, Some(ModifierOrdering { edges: vec![] })] {
        let mut project = create_project(None, true);
        project.modifiers.push(Modifier {
            id: "duplicate".into(),
            name: "Duplicate".into(),
            operations: vec![Operation::AddNotes {
                notes: vec![project.definitions[0].notes[0].clone()],
            }],
        });
        project.instances[0].modifiers.push("duplicate".into());
        project.instances[0].modifier_ordering = ordering;
        validate_project(&project)?;
        invalid(&project);
    }
    Ok(())
}

#[test]
fn slot_replacement_cannot_reuse_an_unaffected_note_id() -> Result<(), CoreError> {
    let mut project = create_project(None, true);
    let mut replacement = project.definitions[0].notes[3].clone();
    replacement.id = project.definitions[0].notes[0].id.clone();
    project.instances[0].overrides.push(Operation::ReplaceSlot {
        slot: "ending-fill".into(),
        notes: vec![replacement],
    });
    validate_project(&project)?;
    invalid(&project);
    Ok(())
}

#[test]
fn finite_transposes_that_overflow_are_rejected_before_serialization() -> Result<(), CoreError> {
    let mut project = create_project(None, true);
    project.instances[0].overrides.extend([
        Operation::Transpose {
            semitones: f64::MAX,
        },
        Operation::Transpose {
            semitones: f64::MAX,
        },
    ]);
    validate_project(&project)?;
    invalid(&project);
    match dispatch(Request {
        id: "explain-overflow".into(),
        principal: Principal::Human,
        state: Some(project),
        method: "project.explain".into(),
        params: json!({"instanceId":"instance-chorus-a"}),
    }) {
        Response::Failure(response) => assert_eq!(response.error.code, "InvalidProject"),
        Response::Success(_) => panic!("non-finite notes escaped the explanation boundary"),
    }
    Ok(())
}

#[test]
fn duplicate_concatenated_ids_across_instances_are_rejected_globally() -> Result<(), CoreError> {
    let mut project = create_project(None, true);
    project.instances.truncate(2);
    project.definitions[0].notes.truncate(1);
    project.definitions[0].notes[0].id = "a@b".into();
    let mut other = project.definitions[0].clone();
    other.id = "other-definition".into();
    other.notes[0].id = "a".into();
    project.instances[0].id = "c".into();
    project.instances[1].id = "b@c".into();
    project.instances[1].definition_id = other.id.clone();
    project.definitions.push(other);
    validate_project(&project)?;
    invalid(&project);
    Ok(())
}

#[test]
fn whole_pattern_replacement_may_remove_transient_overflow_and_duplicates() -> Result<(), CoreError>
{
    let mut project = create_project(None, true);
    project.instances[0].overrides = vec![
        Operation::AddNotes {
            notes: vec![project.definitions[0].notes[0].clone()],
        },
        Operation::Transpose {
            semitones: f64::MAX,
        },
        Operation::Transpose {
            semitones: f64::MAX,
        },
        Operation::ReplaceSlot {
            slot: "notes".into(),
            notes: project.definitions[0].notes.clone(),
        },
    ];
    let rendered = snapshot(&project, None)?;
    assert!(rendered.notes.iter().all(|note| note.pitch.is_finite()));
    Ok(())
}

#[test]
fn duplicate_notes_outside_a_crop_are_not_emitted() -> Result<(), CoreError> {
    let mut project = create_project(None, true);
    project.instances.truncate(1);
    project.instances[0].length = Tick::from_ticks(2 * 960);
    let mut hidden = project.definitions[0].notes[0].clone();
    hidden.start = Tick::from_ticks(6 * 960);
    project.instances[0].overrides.push(Operation::AddNotes {
        notes: vec![hidden],
    });
    let rendered = snapshot(&project, None)?;
    assert_eq!(rendered.notes.len(), 1);
    Ok(())
}

#[test]
fn fractional_pitch_and_large_finite_velocity_factor_survive_repeat_crop() -> Result<(), CoreError>
{
    let mut project = create_project(None, true);
    project.instances.truncate(1);
    project.instances[0].length = Tick::from_ticks(10 * 960 + 480);
    project.definitions[0].notes[0].pitch = 60.25;
    project.instances[0]
        .overrides
        .push(Operation::Velocity { factor: f64::MAX });
    let rendered = snapshot(&project, None)?;
    assert!(rendered.notes.iter().all(|note| note.velocity == 1.0));
    assert_eq!(
        rendered
            .notes
            .iter()
            .filter(|note| note.pitch == 60.25)
            .count(),
        2
    );
    assert!(
        rendered
            .notes
            .iter()
            .any(|note| note.duration == Tick::from_ticks(480))
    );
    Ok(())
}

#[test]
fn dry_run_reports_invalid_local_result_without_mutating_head() {
    let project = create_project(None, true);
    let mut candidate = project.clone();
    candidate.modifiers.push(Modifier {
        id: "duplicate".into(),
        name: "Duplicate".into(),
        operations: vec![Operation::AddNotes {
            notes: vec![project.definitions[0].notes[0].clone()],
        }],
    });
    match dispatch(Request {
        id: "dry-run".into(),
        principal: Principal::Agent,
        state: Some(candidate.clone()),
        method: "change.dryRun".into(),
        params: json!({"expectedRevision":candidate.revision,"command":{
            "type":"applyModifier","instanceId":"instance-chorus-a","modifierId":"duplicate"
        }}),
    }) {
        Response::Success(response) => {
            assert_eq!(response.state, candidate);
            assert_eq!(response.result["executable"], true);
            assert_eq!(response.result["noteResolution"]["ready"], false);
            assert_eq!(response.result["diagnostics"][0]["code"], "InvalidProject");
        }
        Response::Failure(response) => panic!("unexpected failure: {:?}", response.error),
    }
}

#[test]
fn invalid_proposal_preview_does_not_invalidate_main() -> Result<(), CoreError> {
    let mut project = create_project(None, true);
    let proposal = daiw_core::create_proposal(
        &project,
        daiw_core::ProposalParams {
            expected_revision: project.revision.clone(),
            instance_id: project.instances[0].id.clone(),
            request: "duplicate candidate".into(),
            operations: Some(vec![Operation::AddNotes {
                notes: vec![project.definitions[0].notes[0].clone()],
            }]),
        },
    )?;
    let id = proposal.id.clone();
    project.proposals.push(proposal);
    let original = project.clone();
    assert!(matches!(
        snapshot(&project, Some(&id)),
        Err(CoreError::InvalidProject(_))
    ));
    snapshot(&project, None)?;
    assert_eq!(project, original);
    Ok(())
}
