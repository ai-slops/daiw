use daiw_core::{
    AssetId, AudioRegion, HistoryEntry, HistoryId, Modifier, Operation, Principal, RegionId,
    Request, Response, Revision, Tick, TrackId, canonical_hash, create_project, dispatch, snapshot,
    validate_project,
};
use std::fmt::Debug;

fn success<T, E: Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("unexpected error: {error:?}"),
    }
}

fn successful(response: Response) -> Box<daiw_core::SuccessResponse> {
    match response {
        Response::Success(response) => response,
        Response::Failure(response) => panic!("unexpected failure: {}", response.error.message),
    }
}

#[test]
fn project_create_assigns_unique_ids_and_revisions() {
    // Given
    let request = |id: &str| Request {
        id: id.into(),
        principal: Principal::Human,
        state: None,
        method: "project.create".into(),
        params: serde_json::json!({"fixture":false}),
    };

    // When
    let first = successful(dispatch(request("first"))).state;
    let second = successful(dispatch(request("second"))).state;

    // Then
    assert_ne!(first.id, second.id);
    assert_ne!(first.revision, second.revision);
    assert_ne!(first.revision.as_str(), "rev-0");
}

#[test]
fn project_import_assigns_fresh_identity() {
    // Given
    let project = create_project(Some("Imported"), true);
    let original_id = project.id.clone();
    let request = Request {
        id: "import".into(),
        principal: Principal::Human,
        state: None,
        method: "project.import".into(),
        params: serde_json::json!({"project":project}),
    };

    // When
    let imported = successful(dispatch(request)).state;

    // Then
    assert_ne!(imported.id, original_id);
}

#[test]
fn canonical_hash_ignores_revision_and_history() {
    // Given
    let project = create_project(Some("Hash"), true);
    let mut metadata_changed = project.clone();
    metadata_changed.revision = Revision::from("other");
    metadata_changed.history.push(HistoryEntry {
        id: HistoryId::from("history"),
        message: "metadata".into(),
        author: "human".into(),
        revision: Revision::from("other"),
    });

    // When
    let hashes = (
        success(canonical_hash(&project)),
        success(canonical_hash(&metadata_changed)),
    );

    // Then
    assert_eq!(hashes.0, hashes.1);
}

#[test]
fn restore_rejects_stale_expected_revision() {
    // Given
    let project = create_project(Some("Current"), true);
    let request = Request {
        id: "undo".into(),
        principal: Principal::Human,
        state: Some(project.clone()),
        method: "project.undo".into(),
        params: serde_json::json!({"expectedRevision":"stale","snapshot":project}),
    };

    // When
    let response = dispatch(request);

    // Then
    match response {
        Response::Failure(response) => assert_eq!(response.error.code, "StaleRevision"),
        Response::Success(_) => panic!("stale restore succeeded"),
    }
}

#[test]
fn proposal_metadata_mutations_advance_revision() {
    // Given
    let project = create_project(Some("Proposal"), true);
    let create_request = Request {
        id: "proposal".into(),
        principal: Principal::Agent,
        state: Some(project.clone()),
        method: "proposal.create".into(),
        params: serde_json::json!({
            "expectedRevision":project.revision,
            "instanceId":project.instances[0].id,
            "request":"octave up"
        }),
    };

    // When
    let proposed = successful(dispatch(create_request)).state;
    let reject_request = Request {
        id: "reject".into(),
        principal: Principal::Human,
        state: Some(proposed.clone()),
        method: "proposal.reject".into(),
        params: serde_json::json!({
            "expectedRevision":proposed.revision,
            "proposalId":proposed.proposals[0].id
        }),
    };
    let rejected = successful(dispatch(reject_request)).state;

    // Then
    assert_ne!(proposed.revision, project.revision);
    assert_ne!(rejected.revision, proposed.revision);
}

#[test]
fn replace_slot_preserves_notes_outside_named_range() {
    // Given
    let mut project = create_project(Some("Slots"), true);
    project.modifiers.push(Modifier {
        id: "ending".into(),
        name: "Ending".into(),
        operations: vec![Operation::ReplaceSlot {
            slot: "ending-fill".into(),
            notes: vec![daiw_core::Note {
                id: "replacement".into(),
                start: Tick::from_ticks(6 * 960),
                duration: Tick::from_ticks(2 * 960),
                pitch: 90.0,
                velocity: 0.8,
            }],
        }],
    });
    project.instances[0].modifiers.push("ending".into());

    // When
    let notes: Vec<_> = success(snapshot(&project, None))
        .notes
        .into_iter()
        .filter(|note| note.instance_id == project.instances[0].id)
        .collect();

    // Then
    assert_eq!(notes.len(), 4);
    assert!(notes.iter().any(|note| note.pitch == 61.0));
    assert!(notes.iter().any(|note| note.pitch == 90.0));
}

#[test]
fn validation_rejects_region_with_missing_asset() {
    // Given
    let mut project = create_project(Some("Region"), true);
    project.regions.push(AudioRegion {
        id: RegionId::from("region"),
        track_id: TrackId::from("track-fx"),
        asset_id: AssetId::from("missing"),
        start: Tick::from_ticks(0),
        duration: Tick::from_ticks(960),
        gain: 0.0,
        source_offset: 0.0,
    });

    // When
    let error = match validate_project(&project) {
        Ok(()) => panic!("invalid region accepted"),
        Err(error) => error,
    };

    // Then
    assert_eq!(error.code(), "InvalidProject");
}

#[test]
fn explain_result_echoes_instance_id() {
    // Given
    let project = create_project(Some("Explain"), true);
    let instance_id = project.instances[0].id.clone();
    let request = Request {
        id: "explain".into(),
        principal: Principal::Human,
        state: Some(project),
        method: "project.explain".into(),
        params: serde_json::json!({"instanceId":instance_id}),
    };

    // When
    let result = successful(dispatch(request)).result;

    // Then
    assert_eq!(result["instanceId"], instance_id.as_str());
}
