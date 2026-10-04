use daiw_core::{
    ChangeRequest, Command, CoreError, EditScope, Principal, Project, Request, Response,
    apply_change, create_project, dispatch,
};
use serde_json::{Value, json};

fn locally_edited() -> Result<Project, CoreError> {
    let project = create_project(None, true);
    let mut notes = project.definitions[0].notes.clone();
    notes[0].pitch = 49.0;
    apply_change(
        &project,
        Principal::Human,
        ChangeRequest {
            expected_revision: project.revision.clone(),
            command: Command::EditNotes {
                instance_id: project.instances[0].id.clone(),
                scope: EditScope::Instance,
                notes,
            },
        },
    )
}

fn inspect(project: &Project, command: Command) -> Value {
    let response = dispatch(Request {
        id: "inspect".into(),
        principal: Principal::Agent,
        state: Some(project.clone()),
        method: "change.dryRun".into(),
        params: json!({"expectedRevision": project.revision, "command": command}),
    });
    match response {
        Response::Success(response) => {
            assert_eq!(response.state, *project);
            response.result
        }
        Response::Failure(response) => panic!("unexpected failure: {:?}", response.error),
    }
}

#[test]
fn source_deletion_reports_missing_local_target_before_render() -> Result<(), CoreError> {
    let project = locally_edited()?;
    let result = inspect(
        &project,
        Command::EditNotes {
            instance_id: project.instances[0].id.clone(),
            scope: EditScope::Source,
            notes: project.definitions[0].notes[1..].to_vec(),
        },
    );
    assert_eq!(result["executable"], true);
    assert_eq!(result["noteResolution"]["ready"], false);
    assert_eq!(
        result["affectedInstances"],
        json!([
            "instance-chorus-a",
            "instance-chorus-b",
            "instance-chorus-c"
        ])
    );
    let reports = &result["noteResolution"]["instances"];
    assert_eq!(reports[0]["before"]["status"], "clean");
    assert_eq!(reports[0]["after"]["status"], "conflict");
    assert_eq!(
        reports[0]["after"]["target"],
        json!({"kind":"missing", "noteId":"note-chorus-root"})
    );
    assert_eq!(reports[0]["before"]["definitionRevision"], 0);
    assert_eq!(reports[0]["after"]["definitionRevision"], 1);
    assert_eq!(reports[1]["after"]["status"], "clean");
    assert_eq!(result["diagnostics"][0]["entityId"], "instance-chorus-a");
    assert_eq!(result["impact"]["baseRevision"], json!(project.revision));
    assert_eq!(
        result["impact"]["affectedDefinitions"],
        json!(["def-chorus"])
    );
    assert_eq!(
        result["impact"]["previewRanges"][0]["before"],
        json!({"start":24.0,"end":32.0})
    );
    Ok(())
}

#[test]
fn healthy_edit_does_not_inherit_unrelated_instance_conflict() -> Result<(), CoreError> {
    let mut project = locally_edited()?;
    project.definitions[0].notes.remove(0);
    let result = inspect(
        &project,
        Command::EditNotes {
            instance_id: project.instances[1].id.clone(),
            scope: EditScope::Instance,
            notes: project.definitions[0].notes.clone(),
        },
    );
    assert_eq!(result["noteResolution"]["ready"], true);
    assert_eq!(result["affectedInstances"], json!(["instance-chorus-b"]));
    assert_eq!(result["diagnostics"], json!([]));
    Ok(())
}

#[test]
fn corrective_edit_reports_existing_conflict_as_resolved() -> Result<(), CoreError> {
    let mut project = locally_edited()?;
    project.definitions[0].notes.remove(0);
    let result = inspect(
        &project,
        Command::EditNotes {
            instance_id: project.instances[0].id.clone(),
            scope: EditScope::Instance,
            notes: project.definitions[0].notes.clone(),
        },
    );
    let report = &result["noteResolution"]["instances"][0];
    assert_eq!(report["before"]["status"], "conflict");
    assert_eq!(report["after"]["status"], "clean");
    assert_eq!(result["noteResolution"]["ready"], true);
    Ok(())
}

#[test]
fn existing_conflict_remains_distinguishable_from_new_conflict() -> Result<(), CoreError> {
    let mut project = locally_edited()?;
    project.definitions[0].notes.remove(0);
    let result = inspect(
        &project,
        Command::MoveInstance {
            instance_id: project.instances[0].id.clone(),
            start: daiw_core::Tick::from_ticks(960),
            track_id: None,
        },
    );
    let report = &result["noteResolution"]["instances"][0];
    assert_eq!(report["before"]["status"], "conflict");
    assert_eq!(report["before"]["target"], report["after"]["target"]);
    assert_eq!(
        result["impact"]["previewRanges"][0]["after"],
        json!({"start":1.0,"end":9.0})
    );
    Ok(())
}

#[test]
fn invalid_canonical_change_does_not_claim_resolution_was_checked() {
    let project = create_project(None, true);
    let result = inspect(&project, Command::SetTempo { tempo: 0.0 });
    assert_eq!(result["executable"], false);
    assert!(result.get("noteResolution").is_none());
    assert_eq!(result["diagnostics"][0]["code"], "InvalidProject");
}

#[test]
fn deleting_a_broken_instance_reports_absence_after_change() -> Result<(), CoreError> {
    let mut project = locally_edited()?;
    project.definitions[0].notes.remove(0);
    let result = inspect(
        &project,
        Command::DeleteInstance {
            instance_id: project.instances[0].id.clone(),
        },
    );
    assert_eq!(
        result["noteResolution"]["instances"][0]["before"]["status"],
        "conflict"
    );
    assert_eq!(
        result["noteResolution"]["instances"][0]["after"]["status"],
        "absent"
    );
    assert_eq!(result["noteResolution"]["ready"], true);
    Ok(())
}
