use daiw_core::{
    CacheLimits, ChildPlacement, Note, Principal, Project, Request, Response, Tick, WorkerRuntime,
    create_project,
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
    project.definitions[0].length = beat(4.0);
    project.definitions[0].slots.clear();
    project.definitions[0].notes = vec![note("root", 60.0)];
    project.definitions[0].children = vec![ChildPlacement {
        id: "child".into(),
        definition_id: project.definitions[1].id.clone(),
        start: beat(1.0),
        length: beat(2.0),
    }];
    project.definitions[1].length = beat(1.0);
    project.definitions[1].slots.clear();
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
fn repeated_snapshot_is_canonically_equal_and_reports_a_hit() {
    // Given
    let project = nested_project("warm snapshot");
    let mut runtime = WorkerRuntime::default();

    // When
    let cold = result(runtime.dispatch(request(
        "cold",
        project.clone(),
        "project.snapshot",
        json!({}),
    )));
    let warm = result(runtime.dispatch(request(
        "warm",
        project.clone(),
        "project.snapshot",
        json!({}),
    )));
    let stats = stats(&mut runtime, project);

    // Then
    assert_eq!(warm, cold);
    assert_eq!(stats["enabled"], true);
    assert_eq!(stats["totals"]["hits"], 1);
    assert_eq!(stats["totals"]["misses"], 1);
    assert_eq!(stats["lastRequest"]["hits"], 1);
    assert_eq!(stats["resident"]["entries"], 1);
}

#[test]
fn same_revision_changed_leaf_misses_but_unrelated_fields_hit() {
    // Given
    let project = nested_project("key inputs");
    let mut runtime = WorkerRuntime::default();
    let _ = runtime.dispatch(request(
        "cold",
        project.clone(),
        "project.snapshot",
        json!({}),
    ));
    let mut unrelated = project.clone();
    unrelated.name = "renamed".into();
    unrelated.tempo = 91.0;
    unrelated.tracks[0].name = "renamed track".into();
    let mut changed_leaf = unrelated.clone();
    changed_leaf.definitions[1].notes[0].pitch = 73.0;

    // When
    let _ = runtime.dispatch(request(
        "unrelated",
        unrelated,
        "project.snapshot",
        json!({}),
    ));
    let _ = runtime.dispatch(request(
        "leaf",
        changed_leaf.clone(),
        "project.snapshot",
        json!({}),
    ));
    let stats = stats(&mut runtime, changed_leaf);

    // Then
    assert_eq!(stats["totals"]["hits"], 1);
    assert_eq!(stats["totals"]["misses"], 2);
    assert_eq!(stats["resident"]["entries"], 2);
}

#[test]
fn reordered_children_hit_and_cross_project_or_restart_miss() {
    // Given
    let mut project = nested_project("ordering");
    let mut second = project.definitions[0].children[0].clone();
    second.id = "another".into();
    second.start = beat(2.0);
    second.length = beat(1.0);
    project.definitions[0].children.push(second);
    let mut reordered = project.clone();
    reordered.definitions[0].children.reverse();
    let mut other_project = reordered.clone();
    other_project.id = "other-project".into();
    let mut runtime = WorkerRuntime::default();
    let _ = runtime.dispatch(request("cold", project, "project.snapshot", json!({})));

    // When
    let _ = runtime.dispatch(request(
        "reordered",
        reordered.clone(),
        "project.snapshot",
        json!({}),
    ));
    let _ = runtime.dispatch(request(
        "cross",
        other_project,
        "project.snapshot",
        json!({}),
    ));
    let summary = stats(&mut runtime, reordered.clone());
    let restarted = stats(&mut WorkerRuntime::default(), reordered);

    // Then
    assert_eq!(summary["totals"]["hits"], 1);
    assert_eq!(summary["totals"]["misses"], 2);
    assert_eq!(restarted["resident"]["entries"], 0);
    assert_eq!(restarted["totals"]["hits"], 0);
}

#[test]
fn disabled_runtime_stays_empty_and_stats_query_does_not_replace_last_request() {
    // Given
    let project = nested_project("disabled");
    let mut runtime = WorkerRuntime::with_cache_limits(CacheLimits::disabled());
    let _ = runtime.dispatch(request(
        "first",
        project.clone(),
        "project.snapshot",
        json!({}),
    ));
    let before = stats(&mut runtime, project.clone());

    // When
    let after = stats(&mut runtime, project);

    // Then
    assert_eq!(after, before);
    assert_eq!(after["enabled"], false);
    assert_eq!(after["resident"]["entries"], 0);
    assert_eq!(after["totals"]["hits"], 0);
    assert_eq!(after["totals"]["misses"], 0);
}

#[test]
fn agent_cannot_read_cache_stats() {
    // Given
    let project = nested_project("denied stats");
    let mut runtime = WorkerRuntime::default();
    let mut request = request("stats", project, "project.cacheStats", json!({}));
    request.principal = Principal::Agent;

    // When
    let response = runtime.dispatch(request);

    // Then
    match response {
        Response::Failure(response) => assert_eq!(response.error.code, "PermissionDenied"),
        Response::Success(_) => panic!("agent cache stats unexpectedly succeeded"),
    }
}
