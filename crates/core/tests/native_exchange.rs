use daiw_core::{
    Curve, MeterMap, MeterPoint, Principal, Project, Request, Response, TempoMap, TempoPoint, Tick,
    create_project, dispatch,
};
use serde_json::{Value, json};
use std::fmt::Debug;

fn must<T, E: Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("unexpected error: {error:?}"),
    }
}

fn call(state: Option<Project>, method: &str, params: Value) -> Response {
    dispatch(Request {
        id: "exchange".into(),
        principal: Principal::Human,
        state,
        method: method.into(),
        params,
    })
}

fn export(project: &Project) -> Value {
    match call(Some(project.clone()), "project.export", json!({})) {
        Response::Success(response) => {
            assert_eq!(response.state, *project);
            response.result
        }
        Response::Failure(response) => panic!("export failed: {:?}", response.error),
    }
}

fn beat(value: f64) -> Tick {
    must(Tick::from_beats(value))
}

#[test]
fn native_envelope_round_trip_retains_content_and_assigns_new_identity() {
    let project = create_project(None, true);
    let document = export(&project);
    assert_eq!(document["format"], "daiw-project");
    assert_eq!(document["formatVersion"], 1);
    assert_eq!(
        document["requiredFeatures"],
        json!(["core-v1", "named-slots-v1"])
    );
    assert!(document.get("id").is_none());
    assert!(document.get("project").is_none());
    match call(None, "project.import", json!({"project":document})) {
        Response::Success(response) => {
            assert_ne!(response.state.id, project.id);
            assert_eq!(response.state.definitions, project.definitions);
            assert_eq!(response.state.instances, project.instances);
            assert_eq!(response.result["sourceFormatVersion"], 1);
        }
        Response::Failure(response) => panic!("import failed: {:?}", response.error),
    }
}

#[test]
fn unknown_versions_features_and_tampering_never_fall_back_to_raw_import() {
    let project = create_project(None, true);
    let document = export(&project);
    let mut version = document.clone();
    version["formatVersion"] = json!(999);
    let mut feature = document.clone();
    feature["requiredFeatures"] = json!(["future-engine"]);
    let mut corrupt = document.clone();
    corrupt["content"]["name"] = json!("Changed without updating hash");
    let mut missing = document.clone();
    missing["requiredFeatures"] = json!(["core-v1"]);
    for invalid in [version, feature, corrupt, missing] {
        assert!(matches!(
            call(None, "project.import", json!({"project":invalid})),
            Response::Failure(_)
        ));
    }
}

#[test]
fn legacy_unknown_semantic_fields_are_not_silently_discarded() -> Result<(), serde_json::Error> {
    let project = create_project(None, true);
    let mut raw = serde_json::to_value(&project)?;
    raw["definitions"][0]["futureAutomation"] = json!({"gain":12});
    assert!(matches!(
        call(None, "project.import", json!({"project":raw})),
        Response::Failure(_)
    ));
    match call(None, "project.import", json!({"project":project})) {
        Response::Success(response) => assert_eq!(response.result["sourceFormatVersion"], 0),
        Response::Failure(response) => panic!("legacy import failed: {:?}", response.error),
    }
    Ok(())
}

#[test]
fn v1_hash_covers_sorted_full_content_including_revision_and_history() {
    let mut project = create_project(Some("Golden"), false);
    project.id = "p".into();
    project.revision = "r".into();
    let document = export(&project);
    let bytes = br#"{"assets":[],"definitions":[],"history":[],"id":"p","instances":[],"meter":[4,4],"modifiers":[],"name":"Golden","proposals":[],"regions":[],"revision":"r","tempo":112.0,"tracks":[]}"#;
    assert_eq!(
        serde_json::to_vec(&document["content"]).ok().as_deref(),
        Some(bytes.as_slice())
    );
    assert_eq!(
        document["contentHash"],
        format!("blake3:{}", blake3::hash(bytes).to_hex())
    );
    let mut changed = document;
    changed["content"]["revision"] = json!("other-revision");
    match call(None, "project.import", json!({"project":changed})) {
        Response::Failure(response) => assert_eq!(response.error.code, "CorruptProject"),
        Response::Success(_) => panic!("revision tampering passed integrity check"),
    }
}

#[test]
fn timing_maps_round_trip_with_exact_required_features() {
    let mut project = create_project(None, false);
    project.tempo = 100.0;
    project.tempo_map = Some(must(TempoMap::new(vec![
        TempoPoint::new(beat(0.0), 100.0, Curve::Linear),
        TempoPoint::new(beat(4.0), 140.0, Curve::Step),
    ])));
    project.meter = [3, 4];
    project.meter_map = Some(must(MeterMap::new(vec![
        MeterPoint::new(beat(0.0), 3, 4),
        MeterPoint::new(beat(3.0), 4, 4),
    ])));

    let document = export(&project);
    assert_eq!(
        document["requiredFeatures"],
        json!(["core-v1", "meter-map-v1", "tempo-map-v1"])
    );
    match call(None, "project.import", json!({"project":document})) {
        Response::Success(response) => {
            assert_eq!(response.state.tempo_map, project.tempo_map);
            assert_eq!(response.state.meter_map, project.meter_map);
        }
        Response::Failure(response) => panic!("timing map import failed: {:?}", response.error),
    }
}

#[test]
fn timing_map_features_are_required_and_null_maps_remain_noncanonical() {
    let mut project = create_project(None, false);
    project.tempo = 120.0;
    project.tempo_map = Some(must(TempoMap::new(vec![TempoPoint::new(
        beat(0.0),
        120.0,
        Curve::Step,
    )])));
    let document = export(&project);
    let mut missing = document.clone();
    missing["requiredFeatures"] = json!(["core-v1"]);
    assert!(matches!(
        call(None, "project.import", json!({"project":missing})),
        Response::Failure(_)
    ));

    let mut raw = must(serde_json::to_value(create_project(None, false)));
    raw["tempoMap"] = Value::Null;
    raw["meterMap"] = Value::Null;
    assert!(matches!(
        call(None, "project.import", json!({"project":raw})),
        Response::Failure(_)
    ));
}

#[test]
fn empty_optional_legacy_fields_are_allowed_only_on_instances() -> Result<(), serde_json::Error> {
    let project = create_project(None, true);
    let mut raw = serde_json::to_value(&project)?;
    raw["instances"][0]["modifierBindings"] = json!({});
    raw["instances"][0]["modifierOrdering"] = Value::Null;
    assert!(matches!(
        call(None, "project.import", json!({"project":raw.clone()})),
        Response::Success(_)
    ));
    raw["definitions"][0]["modifierBindings"] = json!({});
    assert!(matches!(
        call(None, "project.import", json!({"project":raw})),
        Response::Failure(_)
    ));
    Ok(())
}

#[test]
fn nonzero_finite_pitch_bits_survive_native_json_round_trips() -> Result<(), serde_json::Error> {
    let mut project = create_project(None, true);
    let mut bits = 0x1234_5678_9abc_def0_u64;
    for _ in 0..128 {
        bits = bits.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let pitch = f64::from_bits(bits);
        if !pitch.is_finite() || pitch == 0.0 {
            continue;
        }
        project.definitions[0].notes[0].pitch = pitch;
        let document: Value = serde_json::from_slice(&serde_json::to_vec(&export(&project))?)?;
        match call(None, "project.import", json!({"project":document})) {
            Response::Success(response) => {
                assert_eq!(response.state.definitions[0].notes[0].pitch.to_bits(), bits)
            }
            Response::Failure(response) => panic!(
                "finite value changed during native exchange: {:?}",
                response.error
            ),
        }
    }
    Ok(())
}

#[test]
fn signed_zero_is_canonicalized_for_json_clients() {
    let mut project = create_project(None, true);
    project.definitions[0].notes[0].pitch = -0.0;
    let document = export(&project);
    assert_eq!(
        document["content"]["definitions"][0]["notes"][0]["pitch"]
            .as_f64()
            .map(f64::to_bits),
        Some(0.0_f64.to_bits())
    );
    assert!(matches!(
        call(None, "project.import", json!({"project":document})),
        Response::Success(_)
    ));
}

#[test]
fn unsafe_integer_metadata_is_rejected_instead_of_losing_precision() {
    let mut project = create_project(None, true);
    project.definitions[0].revision = 9_007_199_254_740_993;
    assert!(matches!(
        call(Some(project), "project.export", json!({})),
        Response::Failure(_)
    ));
}
