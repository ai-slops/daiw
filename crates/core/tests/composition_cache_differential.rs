use daiw_core::{
    CacheLimits, ChangeRequest, ChildPlacement, Command, Modifier, Note, Operation, Principal,
    Project, ProposalParams, Request, Response, Tick, WorkerRuntime, apply_change, create_project,
    create_proposal,
};
use serde_json::{Value, json};

fn beat(value: f64) -> Tick {
    Tick::from_beats(value).unwrap_or_else(|error| panic!("invalid test beat: {error}"))
}

fn note(id: &str, start: f64, pitch: f64) -> Note {
    Note {
        id: id.into(),
        start: beat(start),
        duration: beat(0.5),
        pitch,
        velocity: 0.8,
    }
}

fn nested_project() -> Project {
    let mut project = create_project(Some("Differential"), true);
    project.definitions.truncate(2);
    project.instances.truncate(1);
    for definition in &mut project.definitions {
        definition.slots.clear();
    }
    project.definitions[0].length = beat(4.0);
    project.definitions[0].notes = vec![note("root-a", 0.0, 60.0), note("root-b", 0.5, 62.0)];
    project.definitions[0].children = vec![
        ChildPlacement {
            id: "later".into(),
            definition_id: project.definitions[1].id.clone(),
            start: beat(2.0),
            length: beat(1.0),
        },
        ChildPlacement {
            id: "earlier".into(),
            definition_id: project.definitions[1].id.clone(),
            start: beat(1.0),
            length: beat(1.0),
        },
    ];
    project.definitions[1].length = beat(1.0);
    project.definitions[1].notes = vec![note("leaf", 0.0, 72.0)];
    project.definitions[1].children.clear();
    project.instances[0].definition_id = project.definitions[0].id.clone();
    project.instances[0].length = beat(4.0);
    project
}

fn request(project: Project, method: &str, params: Value) -> Request {
    Request {
        id: "differential".into(),
        principal: Principal::Human,
        state: Some(project),
        method: method.into(),
        params,
    }
}

fn encoded(response: Response) -> Vec<u8> {
    serde_json::to_vec(&response)
        .unwrap_or_else(|error| panic!("response serialization failed: {error}"))
}

fn assert_disabled_cold_warm(project: &Project, method: &str, params: Value) {
    let request = request(project.clone(), method, params);
    let disabled = encoded(
        WorkerRuntime::with_cache_limits(CacheLimits::disabled()).dispatch(request.clone()),
    );
    let mut cached = WorkerRuntime::default();
    let cold = encoded(cached.dispatch(request.clone()));
    let warm = encoded(cached.dispatch(request));
    assert_eq!(cold, disabled, "cold {method} response diverged");
    assert_eq!(warm, disabled, "warm {method} response diverged");
}

fn assert_after_warm(base: &Project, changed: &Project, method: &str, params: Value) -> Value {
    let mut cached = WorkerRuntime::default();
    let _ = cached.dispatch(request(base.clone(), method, params.clone()));
    let expected = encoded(
        WorkerRuntime::with_cache_limits(CacheLimits::disabled()).dispatch(request(
            changed.clone(),
            method,
            params.clone(),
        )),
    );
    let actual = encoded(cached.dispatch(request(changed.clone(), method, params)));
    assert_eq!(actual, expected, "changed {method} response diverged");
    match cached.dispatch(request(changed.clone(), "project.cacheStats", json!({}))) {
        Response::Success(response) => response.result,
        Response::Failure(response) => panic!("stats failed: {:?}", response.error),
    }
}

#[test]
fn full_responses_match_for_all_composition_reaching_reads() {
    // Given
    let project = nested_project();
    let instance_id = project.instances[0].id.clone();

    // When / Then
    assert_disabled_cold_warm(&project, "project.snapshot", json!({}));
    assert_disabled_cold_warm(
        &project,
        "project.explain",
        json!({"instanceId": instance_id}),
    );
    assert_disabled_cold_warm(
        &project,
        "project.queryNotes",
        json!({"instanceId": instance_id, "scope": "instance", "selector": {"type": "all"}, "maxResults": 32}),
    );
    assert_disabled_cold_warm(
        &project,
        "change.dryRun",
        json!({"expectedRevision": project.revision, "command": {"type": "setTempo", "tempo": 123.0}}),
    );
}

#[test]
fn excluded_metadata_hits_while_current_explain_metadata_is_returned() {
    // Given
    let base = nested_project();
    let mut changed = base.clone();
    changed.definitions[0].name = "Current name".into();
    changed.definitions[0].slots.insert(
        "current".into(),
        daiw_core::NoteSlot {
            start: beat(0.0),
            end: beat(1.0),
        },
    );
    let instance_id = base.instances[0].id.clone();

    // When
    let stats = assert_after_warm(
        &base,
        &changed,
        "project.explain",
        json!({"instanceId": instance_id}),
    );

    // Then
    assert_eq!(stats["lastRequest"]["hits"], 1);
}

#[test]
fn child_order_hits_but_flat_note_order_and_descendant_revision_miss() {
    // Given
    let base = nested_project();
    let mut reversed = base.clone();
    reversed.definitions[0].children.reverse();
    let mut flat_base = base.clone();
    flat_base.definitions[0].children.clear();
    let mut reordered_notes = flat_base.clone();
    reordered_notes.definitions[0].notes.reverse();
    let mut revised = base.clone();
    revised.definitions[1].revision += 1;

    // When
    let reversed_stats = assert_after_warm(&base, &reversed, "project.snapshot", json!({}));
    let note_stats = assert_after_warm(&flat_base, &reordered_notes, "project.snapshot", json!({}));
    let revision_stats = assert_after_warm(&base, &revised, "project.snapshot", json!({}));

    // Then
    assert_eq!(reversed_stats["lastRequest"]["hits"], 1);
    assert_eq!(note_stats["lastRequest"]["misses"], 1);
    assert_eq!(revision_stats["lastRequest"]["misses"], 1);
}

#[test]
fn stale_binding_and_proposal_failures_are_cache_equivalent() {
    // Given
    let mut project = nested_project();
    project.modifiers.push(Modifier {
        id: "replace".into(),
        name: "Replace".into(),
        operations: vec![Operation::ReplaceSlot {
            slot: "notes".into(),
            notes: vec![note("replacement", 0.0, 67.0)],
        }],
    });
    let attached = apply_change(
        &project,
        Principal::Human,
        ChangeRequest {
            expected_revision: project.revision.clone(),
            command: Command::ApplyModifier {
                instance_id: project.instances[0].id.clone(),
                modifier_id: "replace".into(),
                ordering: None,
            },
        },
    )
    .unwrap_or_else(|error| panic!("attachment failed: {error:?}"));
    let mut stale_binding = attached.clone();
    stale_binding.definitions[1].revision += 1;
    let proposal = create_proposal(
        &project,
        ProposalParams {
            expected_revision: project.revision.clone(),
            instance_id: project.instances[0].id.clone(),
            request: "explicit".into(),
            operations: Some(vec![Operation::ReplaceSlot {
                slot: "notes".into(),
                notes: vec![note("proposal", 0.0, 69.0)],
            }]),
        },
    )
    .unwrap_or_else(|error| panic!("proposal failed: {error:?}"));
    let proposal_id = proposal.id.clone();
    let mut stale_proposal = project;
    stale_proposal.proposals.push(proposal);
    stale_proposal.definitions[1].revision += 1;

    // When / Then
    assert_disabled_cold_warm(&stale_binding, "project.snapshot", json!({}));
    assert_disabled_cold_warm(
        &stale_proposal,
        "project.snapshot",
        json!({"proposalId": proposal_id}),
    );
}
