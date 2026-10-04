use daiw_core::{
    BindingMismatch, ChangeRequest, Command, CoreError, Operation, Principal, Project, Request,
    Response, Tick, apply_change, create_project, dispatch, snapshot, validate_project,
};
use serde_json::json;

fn bound() -> Result<Project, CoreError> {
    let mut project = create_project(None, true);
    project.modifiers.push(daiw_core::Modifier {
        id: "fill".into(),
        name: "Fill".into(),
        operations: vec![Operation::ReplaceSlot {
            slot: "ending-fill".into(),
            notes: vec![project.definitions[0].notes[3].clone()],
        }],
    });
    change(
        &project,
        Command::ApplyModifier {
            instance_id: "instance-chorus-a".into(),
            modifier_id: "fill".into(),
            ordering: None,
        },
    )
}

fn change(project: &Project, command: Command) -> Result<Project, CoreError> {
    apply_change(
        project,
        Principal::Human,
        ChangeRequest {
            expected_revision: project.revision.clone(),
            command,
        },
    )
}

fn rebase() -> Command {
    Command::RebaseModifier {
        instance_id: "instance-chorus-a".into(),
        modifier_id: "fill".into(),
    }
}

fn reason(project: &Project) -> BindingMismatch {
    match snapshot(project, None) {
        Err(CoreError::ModifierBinding(issue)) => issue.reason,
        outcome => panic!("expected binding conflict: {outcome:?}"),
    }
}

#[test]
fn same_revision_cannot_hide_slot_range_or_target_drift() -> Result<(), CoreError> {
    let original = bound()?;
    let mut project = original.clone();
    if let Some(slot) = project.definitions[0].slots.get_mut("ending-fill") {
        slot.start = Tick::from_ticks(5 * 960);
    }
    assert_eq!(reason(&project), BindingMismatch::RangeChanged);
    let mut project = original.clone();
    project.definitions[0].notes[3].id = "replacement-source-id".into();
    assert_eq!(reason(&project), BindingMismatch::TargetsChanged);
    let mut project = original.clone();
    project.definitions[0].notes.pop();
    assert_eq!(reason(&project), BindingMismatch::CardinalityChanged);
    let mut project = original;
    project.definitions[0].slots.clear();
    assert_eq!(reason(&project), BindingMismatch::SlotMissing);
    assert!(change(&project, rebase()).is_err());
    Ok(())
}

#[test]
fn duplicate_copies_binding_and_remove_cleans_only_its_attachment() -> Result<(), CoreError> {
    let project = bound()?;
    let duplicate = change(
        &project,
        Command::DuplicateInstance {
            instance_id: "instance-chorus-a".into(),
        },
    )?;
    let last = duplicate.instances.len() - 1;
    assert_eq!(
        duplicate.instances[last].modifier_bindings,
        project.instances[0].modifier_bindings
    );
    let removed = change(
        &duplicate,
        Command::RemoveModifier {
            instance_id: "instance-chorus-a".into(),
            modifier_id: "fill".into(),
        },
    )?;
    assert!(removed.instances[0].modifier_bindings.is_empty());
    assert!(removed.instances[0].modifiers.is_empty());
    assert_eq!(
        removed.instances[last].modifier_bindings,
        project.instances[0].modifier_bindings
    );
    Ok(())
}

#[test]
fn malformed_metadata_is_rejected_but_stale_metadata_is_saveable() -> Result<(), CoreError> {
    let original = bound()?;
    let mut project = original.clone();
    for binding in project.instances[0].modifier_bindings.values_mut() {
        for slot in binding.slots.values_mut() {
            slot.cardinality += 1;
        }
    }
    assert!(validate_project(&project).is_err());
    let mut project = original.clone();
    project.instances[0].modifiers.clear();
    assert!(validate_project(&project).is_err());
    let mut project = original;
    project.definitions[0].revision += 1;
    validate_project(&project)?;
    assert_eq!(reason(&project), BindingMismatch::SourceRevisionChanged);
    Ok(())
}

#[test]
fn rebase_is_revision_bound_and_human_only() -> Result<(), CoreError> {
    let project = bound()?;
    let denied = apply_change(
        &project,
        Principal::Agent,
        ChangeRequest {
            expected_revision: project.revision.clone(),
            command: rebase(),
        },
    );
    assert!(matches!(denied, Err(CoreError::PermissionDenied)));
    let stale = apply_change(
        &project,
        Principal::Human,
        ChangeRequest {
            expected_revision: "old".into(),
            command: rebase(),
        },
    );
    assert!(matches!(stale, Err(CoreError::StaleRevision { .. })));
    Ok(())
}

#[test]
fn explicit_rebase_upgrades_legacy_and_dry_run_remains_non_mutating() -> Result<(), CoreError> {
    let mut project = bound()?;
    project.instances[0].modifier_bindings.clear();
    let upgraded = change(&project, rebase())?;
    assert_eq!(upgraded.instances[0].modifier_bindings.len(), 1);
    let mut stale = upgraded;
    stale.definitions[0].revision += 1;
    match dispatch(Request {
        id: "rebase-preview".into(),
        principal: Principal::Agent,
        state: Some(stale.clone()),
        method: "change.dryRun".into(),
        params: json!({"expectedRevision":stale.revision,"command":rebase()}),
    }) {
        Response::Success(response) => {
            assert_eq!(response.state, stale);
            let report = &response.result["noteResolution"]["instances"][0];
            assert_eq!(
                report["before"]["binding"],
                json!({"modifierId":"fill","reason":"sourceRevisionChanged"})
            );
            assert_eq!(report["before"]["actions"], json!(["rebaseModifier"]));
            assert_eq!(report["after"]["status"], "clean");
        }
        Response::Failure(response) => panic!("unexpected failure: {:?}", response.error),
    }
    Ok(())
}

#[test]
fn virtual_whole_pattern_binding_and_missing_metadata_round_trip()
-> Result<(), Box<dyn std::error::Error>> {
    let mut project = bound()?;
    project.instances[0].modifiers.clear();
    project.instances[0].modifier_bindings.clear();
    project.modifiers[0].operations = vec![Operation::ReplaceSlot {
        slot: "notes".into(),
        notes: project.definitions[0].notes.clone(),
    }];
    let project = change(
        &project,
        Command::ApplyModifier {
            instance_id: "instance-chorus-a".into(),
            modifier_id: "fill".into(),
            ordering: None,
        },
    )?;
    let mut wire = serde_json::to_value(&project)?;
    assert_eq!(
        wire["instances"][0]["modifierBindings"]["fill"]["slots"]["notes"]["cardinality"],
        4
    );
    if let Some(instance) = wire["instances"][0].as_object_mut() {
        instance.remove("modifierBindings");
    }
    let legacy: Project = serde_json::from_value(wire)?;
    assert!(legacy.instances[0].modifier_bindings.is_empty());
    snapshot(&legacy, None)?;
    Ok(())
}

#[test]
fn rebase_does_not_claim_to_fix_unrelated_local_patch_errors() -> Result<(), CoreError> {
    let mut project = bound()?;
    project.definitions[0].revision += 1;
    let mut note = project.definitions[0].notes[0].clone();
    note.id = "missing-local-target".into();
    project.instances[0]
        .overrides
        .push(Operation::PatchNotes(daiw_core::NotePatch {
            updates: vec![note],
            added: vec![],
            removed: vec![],
        }));
    let rebased = change(&project, rebase())?;
    assert!(matches!(
        snapshot(&rebased, None),
        Err(CoreError::NoteTarget(_))
    ));
    Ok(())
}
