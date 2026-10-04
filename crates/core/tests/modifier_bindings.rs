use daiw_core::{
    Operation, Principal, Project, Request, Response, create_project, dispatch, snapshot,
};
use serde_json::{Value, json};

fn fixture() -> Project {
    let mut project = create_project(None, true);
    project.modifiers.push(daiw_core::Modifier {
        id: "bound-fill".into(),
        name: "Bound fill".into(),
        operations: vec![Operation::ReplaceSlot {
            slot: "ending-fill".into(),
            notes: vec![project.definitions[0].notes[3].clone()],
        }],
    });
    project
}

fn apply(project: &Project, command: Value) -> Project {
    match dispatch(Request {
        id: "binding-test".into(),
        principal: Principal::Human,
        state: Some(project.clone()),
        method: "change.apply".into(),
        params: json!({"expectedRevision":project.revision,"command":command}),
    }) {
        Response::Success(response) => response.state,
        Response::Failure(response) => panic!("unexpected failure: {:?}", response.error),
    }
}

fn attach(project: &Project) -> Project {
    apply(
        project,
        json!({"type":"applyModifier","instanceId":"instance-chorus-a","modifierId":"bound-fill"}),
    )
}

fn change_source(project: &Project) -> Project {
    let mut notes = project.definitions[0].notes.clone();
    notes[0].pitch += 1.0;
    apply(
        project,
        json!({"type":"editNotes","instanceId":"instance-chorus-a","scope":"source","notes":notes}),
    )
}

#[test]
fn new_attachment_persists_source_revision_targets_count_and_range()
-> Result<(), Box<dyn std::error::Error>> {
    let project = attach(&fixture());
    let wire = serde_json::to_value(&project)?;
    let binding = &wire["instances"][0]["modifierBindings"]["bound-fill"];
    assert_eq!(binding["definitionId"], "def-chorus");
    assert_eq!(binding["definitionRevision"], 0);
    assert_eq!(
        binding["slots"]["ending-fill"]["noteIds"],
        json!(["note-chorus-four"])
    );
    assert_eq!(binding["slots"]["ending-fill"]["cardinality"], 1);
    assert_eq!(
        binding["slots"]["ending-fill"]["range"],
        json!({"start":6.0,"end":8.0})
    );
    snapshot(&project, None)?;
    Ok(())
}

#[test]
fn source_edit_requires_explicit_rebase_even_if_target_ids_stay_same() {
    let project = change_source(&attach(&fixture()));
    match snapshot(&project, None) {
        Ok(_) => panic!("stale binding must block resolution"),
        Err(error) => assert_eq!(error.code(), "RebaseConflict"),
    }
}

#[test]
fn rebase_refreshes_metadata_without_rewriting_source_or_modifier()
-> Result<(), Box<dyn std::error::Error>> {
    let project = change_source(&attach(&fixture()));
    let rebased = apply(
        &project,
        json!({"type":"rebaseModifier","instanceId":"instance-chorus-a","modifierId":"bound-fill"}),
    );
    assert_eq!(rebased.definitions, project.definitions);
    assert_eq!(rebased.modifiers, project.modifiers);
    assert_eq!(
        rebased.instances[0].overrides,
        project.instances[0].overrides
    );
    let wire = serde_json::to_value(&rebased)?;
    assert_eq!(
        wire["instances"][0]["modifierBindings"]["bound-fill"]["definitionRevision"],
        1
    );
    let restored: Project = serde_json::from_value(wire)?;
    assert_eq!(snapshot(&restored, None)?, snapshot(&rebased, None)?);
    Ok(())
}

#[test]
fn repeated_apply_cannot_silently_rebind_a_stale_attachment() {
    let project = change_source(&attach(&fixture()));
    let reapplied = attach(&project);
    assert_eq!(reapplied.instances, project.instances);
    assert!(snapshot(&reapplied, None).is_err());
}

#[test]
fn legacy_slot_attachment_is_readable_but_explicitly_unbound() {
    let mut project = fixture();
    project.instances[0].modifiers.push("bound-fill".into());
    match dispatch(Request {
        id: "legacy".into(),
        principal: Principal::Human,
        state: Some(project),
        method: "project.explain".into(),
        params: json!({"instanceId":"instance-chorus-a"}),
    }) {
        Response::Success(response) => {
            assert_eq!(response.result["unboundModifierIds"], json!(["bound-fill"]))
        }
        Response::Failure(response) => panic!("legacy must remain readable: {:?}", response.error),
    }
}
