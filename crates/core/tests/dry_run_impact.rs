use daiw_core::{
    Command, Operation, Principal, Project, Request, Response, Tick, create_project, dispatch,
};
use serde_json::{Value, json};

fn inspect(project: &Project, command: Command) -> Value {
    match dispatch(Request {
        id: "impact".into(),
        principal: Principal::Agent,
        state: Some(project.clone()),
        method: "change.dryRun".into(),
        params: json!({"expectedRevision":project.revision,"command":command}),
    }) {
        Response::Success(response) => {
            assert_eq!(response.state, *project);
            response.result
        }
        Response::Failure(response) => panic!("unexpected failure: {:?}", response.error),
    }
}

#[test]
fn new_instance_is_included_with_no_previous_range() {
    let project = create_project(None, true);
    let result = inspect(
        &project,
        Command::Instantiate {
            definition_id: project.definitions[0].id.clone(),
            track_id: project.tracks[3].id.clone(),
            start: Tick::from_ticks(960),
            name: None,
        },
    );
    assert_eq!(
        result["affectedInstances"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(
        result["noteResolution"]["instances"][0]["before"]["status"],
        "absent"
    );
    assert_eq!(
        result["noteResolution"]["instances"][0]["after"]["noteCount"],
        4
    );
    assert_eq!(result["impact"]["previewRanges"][0]["before"], Value::Null);
    assert_eq!(
        result["impact"]["previewRanges"][0]["after"],
        json!({"start":1.0,"end":9.0})
    );
}

#[test]
fn tempo_change_covers_all_instances_but_rename_covers_none() {
    let project = create_project(None, true);
    let tempo = inspect(&project, Command::SetTempo { tempo: 120.0 });
    let rename = inspect(
        &project,
        Command::RenameProject {
            name: "Renamed".into(),
        },
    );
    assert_eq!(
        tempo["affectedInstances"].as_array().map(Vec::len),
        Some(project.instances.len())
    );
    assert_eq!(rename["affectedInstances"], json!([]));
    assert_eq!(rename["noteResolution"]["instances"], json!([]));
}

#[test]
fn local_addition_collision_returns_machine_readable_target() {
    let mut project = create_project(None, true);
    let mut added = project.definitions[0].notes[0].clone();
    added.id = "local-added".into();
    project.instances[0]
        .overrides
        .push(Operation::PatchNotes(daiw_core::NotePatch {
            updates: vec![],
            added: vec![added.clone()],
            removed: vec![],
        }));
    let mut notes = project.definitions[0].notes.clone();
    notes.push(added);
    let result = inspect(
        &project,
        Command::EditNotes {
            instance_id: project.instances[0].id.clone(),
            scope: daiw_core::EditScope::Source,
            notes,
        },
    );
    assert_eq!(
        result["noteResolution"]["instances"][0]["after"]["target"],
        json!({"kind":"addedIdCollision","noteId":"local-added"})
    );
    assert_eq!(
        result["noteResolution"]["instances"][0]["after"]["actions"],
        json!(["chooseUniqueNoteIds", "editInstanceNotes"])
    );
}

#[test]
fn ambiguous_modifier_target_is_not_relabelled_as_missing() {
    let mut project = create_project(None, true);
    let note = project.definitions[0].notes[0].clone();
    project.instances[0]
        .overrides
        .push(Operation::PatchNotes(daiw_core::NotePatch {
            updates: vec![note.clone()],
            added: vec![],
            removed: vec![],
        }));
    project.modifiers.push(daiw_core::Modifier {
        id: "duplicate-note".into(),
        name: "Duplicate".into(),
        operations: vec![Operation::AddNotes { notes: vec![note] }],
    });
    let result = inspect(
        &project,
        Command::ApplyModifier {
            instance_id: project.instances[0].id.clone(),
            modifier_id: "duplicate-note".into(),
            ordering: None,
        },
    );
    assert_eq!(
        result["noteResolution"]["instances"][0]["before"]["status"],
        "clean"
    );
    assert_eq!(
        result["noteResolution"]["instances"][0]["after"]["target"],
        json!({"kind":"ambiguous","noteId":"note-chorus-root"})
    );
}

#[test]
fn stale_request_returns_only_structural_failure_without_speculative_report() {
    let project = create_project(None, true);
    match dispatch(Request {
        id: "stale".into(),
        principal: Principal::Agent,
        state: Some(project.clone()),
        method: "change.dryRun".into(),
        params: json!({"expectedRevision":"stale","command":{"type":"setTempo","tempo":120}}),
    }) {
        Response::Success(response) => {
            assert_eq!(response.state, project);
            assert_eq!(response.result["executable"], false);
            assert_eq!(response.result["diagnostics"][0]["code"], "StaleRevision");
            assert!(response.result.get("impact").is_none());
            assert!(response.result.get("noteResolution").is_none());
        }
        Response::Failure(response) => panic!("unexpected failure: {:?}", response.error),
    }
}
