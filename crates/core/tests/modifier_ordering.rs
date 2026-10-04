use daiw_core::{
    Operation, Principal, Project, Request, Response, create_project, dispatch, snapshot,
};
use serde_json::{Value, json};

fn fixture() -> Project {
    let mut project = create_project(None, true);
    let mut added = project.definitions[0].notes[0].clone();
    added.id = "added-note".into();
    added.pitch = 60.0;
    project.modifiers = vec![
        daiw_core::Modifier {
            id: "transpose".into(),
            name: "Transpose".into(),
            operations: vec![Operation::Transpose { semitones: 12.0 }],
        },
        daiw_core::Modifier {
            id: "add".into(),
            name: "Add".into(),
            operations: vec![Operation::AddNotes { notes: vec![added] }],
        },
    ];
    project.instances[0].modifiers = vec!["transpose".into(), "add".into()];
    project
}

fn request(project: &Project, edges: Value, method: &str) -> Response {
    dispatch(Request {
        id: "ordering".into(),
        principal: Principal::Human,
        state: Some(project.clone()),
        method: method.into(),
        params: json!({"expectedRevision":project.revision,"command":{
            "type":"setModifierOrdering","instanceId":"instance-chorus-a","ordering":{"edges":edges}
        }}),
    })
}

fn changed(response: Response) -> Project {
    match response {
        Response::Success(response) => response.state,
        Response::Failure(response) => panic!("unexpected failure: {:?}", response.error),
    }
}

#[test]
fn explicit_edge_changes_execution_without_reordering_attachment_array()
-> Result<(), Box<dyn std::error::Error>> {
    let project = fixture();
    let ordered = changed(request(
        &project,
        json!([{"before":"add","after":"transpose"}]),
        "change.apply",
    ));
    assert_eq!(
        ordered.instances[0].modifiers,
        project.instances[0].modifiers
    );
    let rendered = snapshot(&ordered, None)?;
    assert!(
        rendered
            .notes
            .iter()
            .any(|note| note.origin.note_id.as_str() == "added-note" && note.pitch == 72.0)
    );
    let wire = serde_json::to_value(&ordered)?;
    assert_eq!(
        wire["instances"][0]["modifierOrdering"]["edges"],
        json!([{"before":"add","after":"transpose"}])
    );
    let restored: Project = serde_json::from_value(wire)?;
    assert_eq!(snapshot(&restored, None)?, rendered);
    Ok(())
}

#[test]
fn explicit_empty_is_not_a_legacy_order_fallback() {
    let project = fixture();
    match request(&project, json!([]), "change.dryRun") {
        Response::Success(response) => {
            assert_eq!(response.state, project);
            assert_eq!(response.result["executable"], false);
            assert_eq!(
                response.result["diagnostics"][0]["code"],
                "ModifierOrderRequired"
            );
        }
        Response::Failure(response) => panic!("expected dry-run report: {:?}", response.error),
    }
}

#[test]
fn cycle_is_rejected_without_returning_replacement_state() {
    let project = fixture();
    match request(
        &project,
        json!([{"before":"add","after":"transpose"},{"before":"transpose","after":"add"}]),
        "change.apply",
    ) {
        Response::Failure(response) => assert_eq!(response.error.code, "ModifierOrderingCycle"),
        Response::Success(_) => panic!("cyclic graph applied"),
    }
}

#[test]
fn legacy_execution_order_and_absent_metadata_are_preserved()
-> Result<(), Box<dyn std::error::Error>> {
    let project = fixture();
    let wire = serde_json::to_value(&project)?;
    assert!(wire["instances"][0].get("modifierOrdering").is_none());
    let rendered = snapshot(&project, None)?;
    assert!(
        rendered
            .notes
            .iter()
            .any(|note| note.origin.note_id.as_str() == "added-note" && note.pitch == 60.0)
    );
    Ok(())
}

#[test]
fn explanation_exposes_core_derived_access_and_execution_plan() {
    let project = fixture();
    match dispatch(Request {
        id: "plan".into(),
        principal: Principal::Agent,
        state: Some(project),
        method: "project.explain".into(),
        params: json!({"instanceId":"instance-chorus-a"}),
    }) {
        Response::Success(response) => {
            let plan = &response.result["modifierPlan"];
            assert_eq!(plan["mode"], "legacy");
            assert_eq!(plan["order"], json!(["transpose", "add"]));
            assert_eq!(
                plan["modifiers"][0]["writes"],
                json!([{"kind":"notePitch"}])
            );
            assert!(
                plan["modifiers"][0]["reads"]
                    .as_array()
                    .is_some_and(|reads| reads.contains(&json!({"kind":"noteMembership"})))
            );
        }
        Response::Failure(response) => panic!("unexpected failure: {:?}", response.error),
    }
}
