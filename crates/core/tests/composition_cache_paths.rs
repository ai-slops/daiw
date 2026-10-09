use daiw_core::{
    ChildPlacement, Modifier, Note, Operation, Principal, Project, Request, Response, Tick,
    WorkerRuntime, create_project,
};
use serde_json::{Value, json};

fn beat(value: f64) -> Tick {
    Tick::from_beats(value).unwrap_or_else(|error| panic!("invalid test beat: {error}"))
}

fn note(id: &str, pitch: f64) -> Note {
    Note {
        id: id.into(),
        start: beat(0.0),
        duration: beat(0.5),
        pitch,
        velocity: 0.8,
    }
}

fn nested_project(name: &str) -> Project {
    let mut project = create_project(Some(name), true);
    project.definitions.truncate(2);
    project.instances.truncate(1);
    for definition in &mut project.definitions {
        definition.slots.clear();
    }
    project.definitions[0].length = beat(4.0);
    project.definitions[0].notes = vec![note("root", 60.0)];
    project.definitions[0].children = vec![ChildPlacement {
        id: "child".into(),
        definition_id: project.definitions[1].id.clone(),
        start: beat(1.0),
        length: beat(2.0),
    }];
    project.definitions[1].length = beat(1.0);
    project.definitions[1].notes = vec![note("leaf", 72.0)];
    project.definitions[1].children.clear();
    project.instances[0].definition_id = project.definitions[0].id.clone();
    project.instances[0].length = beat(4.0);
    project
}

fn request(id: &str, project: Project, method: &str, params: Value) -> Request {
    Request {
        id: id.into(),
        principal: Principal::Human,
        state: Some(project),
        method: method.into(),
        params,
    }
}

fn result(response: Response) -> Value {
    match response {
        Response::Success(response) => response.result,
        Response::Failure(response) => panic!("request failed: {:?}", response.error),
    }
}

fn stats(runtime: &mut WorkerRuntime, project: Project) -> Value {
    result(runtime.dispatch(request("stats", project, "project.cacheStats", json!({}))))
}

#[test]
fn explain_query_proposal_and_mutation_paths_reuse_composition() {
    // Given
    let mut project = nested_project("all paths");
    project.modifiers.push(Modifier {
        id: "cache-modifier".into(),
        name: "Cache modifier".into(),
        operations: vec![Operation::ReplaceSlot {
            slot: "notes".into(),
            notes: vec![note("replacement", 65.0)],
        }],
    });
    let instance_id = project.instances[0].id.clone();
    let mut runtime = WorkerRuntime::default();
    let calls = [
        ("project.explain", json!({"instanceId": instance_id})),
        (
            "project.queryNotes",
            json!({"instanceId": instance_id, "scope": "instance", "selector": {"type": "all"}, "maxResults": 16}),
        ),
        (
            "proposal.create",
            json!({"expectedRevision": project.revision, "instanceId": instance_id, "request": "explicit", "operations": [{"type": "transpose", "semitones": 1.0}]}),
        ),
        (
            "change.apply",
            json!({"expectedRevision": project.revision, "command": {"type": "applyModifier", "instanceId": instance_id, "modifierId": "cache-modifier"}}),
        ),
        (
            "change.dryRun",
            json!({"expectedRevision": project.revision, "command": {"type": "applyModifier", "instanceId": instance_id, "modifierId": "cache-modifier"}}),
        ),
    ];

    // When
    for (method, params) in calls {
        let first = runtime.dispatch(request("first", project.clone(), method, params.clone()));
        let second = runtime.dispatch(request("second", project.clone(), method, params));
        assert!(matches!(first, Response::Success(_)));
        assert!(matches!(second, Response::Success(_)));
    }
    let stats = stats(&mut runtime, project);

    // Then
    assert!(
        stats["totals"]["hits"]
            .as_u64()
            .is_some_and(|hits| hits >= 7)
    );
    assert!(
        stats["totals"]["misses"]
            .as_u64()
            .is_some_and(|misses| misses >= 1)
    );
}

#[test]
fn local_operations_hit_cached_base_but_update_output() {
    // Given
    let project = nested_project("local operations");
    let mut changed = project.clone();
    changed.instances[0]
        .overrides
        .push(Operation::Transpose { semitones: 5.0 });
    let mut runtime = WorkerRuntime::default();
    let cold = result(runtime.dispatch(request("cold", project, "project.snapshot", json!({}))));

    // When
    let warm = result(runtime.dispatch(request(
        "warm",
        changed.clone(),
        "project.snapshot",
        json!({}),
    )));
    let stats = stats(&mut runtime, changed);

    // Then
    assert_ne!(warm["snapshot"]["notes"], cold["snapshot"]["notes"]);
    assert_eq!(stats["lastRequest"]["hits"], 1);
}

#[test]
fn descendant_revision_change_misses_and_updates_provenance() {
    // Given
    let project = nested_project("provenance");
    let mut changed = project.clone();
    changed.definitions[1].revision += 1;
    let instance_id = project.instances[0].id.clone();
    let mut runtime = WorkerRuntime::default();
    let _ = runtime.dispatch(request(
        "cold",
        project,
        "project.explain",
        json!({"instanceId": instance_id}),
    ));

    // When
    let explanation = result(runtime.dispatch(request(
        "changed",
        changed.clone(),
        "project.explain",
        json!({"instanceId": instance_id}),
    )));
    let stats = stats(&mut runtime, changed);

    // Then
    assert_eq!(stats["totals"]["misses"], 2);
    assert!(
        explanation["compositionNotes"]
            .as_array()
            .is_some_and(|notes| notes.iter().any(|note| {
                note["origin"]["definitionPath"]
                    .as_array()
                    .is_some_and(|path| path.iter().any(|entry| entry["definitionRevision"] == 1))
            }))
    );
}
