use daiw_core::{
    ChangeRequest, Command, CoreError, Curve, MeterMap, MeterPoint, Principal, Request, Response,
    TempoMap, TempoPoint, Tick, apply_change, create_project, dispatch,
};
use serde_json::{json, to_value};
use std::fmt::Debug;

fn must<T, E: Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("unexpected error: {error:?}"),
    }
}

fn beat(value: f64) -> Tick {
    must(Tick::from_beats(value))
}

fn tempo_map() -> TempoMap {
    must(TempoMap::new(vec![
        TempoPoint::new(beat(0.0), 100.0, Curve::Linear),
        TempoPoint::new(beat(4.0), 140.0, Curve::Step),
    ]))
}

fn meter_map() -> MeterMap {
    must(MeterMap::new(vec![
        MeterPoint::new(beat(0.0), 3, 4),
        MeterPoint::new(beat(3.0), 4, 4),
    ]))
}

#[test]
fn timing_commands_have_canonical_wire_shapes() {
    assert_eq!(
        must(to_value(Command::SetTempoMap {
            tempo_map: tempo_map()
        })),
        json!({"type":"setTempoMap","tempoMap":{"points":[
            {"at":0.0,"bpm":100.0,"curve":"linear"},
            {"at":4.0,"bpm":140.0,"curve":"step"}
        ]}})
    );
    assert_eq!(
        must(to_value(Command::SetMeter { meter: [7, 8] })),
        json!({"type":"setMeter","meter":[7,8]})
    );
    assert_eq!(
        must(to_value(Command::SetMeterMap {
            meter_map: meter_map()
        })),
        json!({"type":"setMeterMap","meterMap":{"points":[
            {"at":0.0,"numerator":3,"denominator":4},
            {"at":3.0,"numerator":4,"denominator":4}
        ]}})
    );
}

#[test]
fn map_setters_sync_scalars_and_scalar_setters_clear_maps() {
    let project = create_project(None, false);
    let changed = must(apply_change(
        &project,
        Principal::Human,
        ChangeRequest {
            expected_revision: project.revision.clone(),
            command: Command::SetTempoMap {
                tempo_map: tempo_map(),
            },
        },
    ));
    assert_eq!(changed.tempo, 100.0);
    assert!(changed.tempo_map.is_some());

    let cleared = must(apply_change(
        &changed,
        Principal::Human,
        ChangeRequest {
            expected_revision: changed.revision.clone(),
            command: Command::SetTempo { tempo: 123.0 },
        },
    ));
    assert_eq!(cleared.tempo, 123.0);
    assert!(cleared.tempo_map.is_none());
}

#[test]
fn meter_commands_sync_and_clear_maps() {
    let project = create_project(None, false);
    let changed = must(apply_change(
        &project,
        Principal::Human,
        ChangeRequest {
            expected_revision: project.revision.clone(),
            command: Command::SetMeterMap {
                meter_map: meter_map(),
            },
        },
    ));
    assert_eq!(changed.meter, [3, 4]);
    assert!(changed.meter_map.is_some());

    let cleared = must(apply_change(
        &changed,
        Principal::Human,
        ChangeRequest {
            expected_revision: changed.revision.clone(),
            command: Command::SetMeter { meter: [7, 8] },
        },
    ));
    assert_eq!(cleared.meter, [7, 8]);
    assert!(cleared.meter_map.is_none());
}

#[test]
fn timing_maps_are_global_impacts() {
    let project = create_project(None, true);
    let response = dispatch(Request {
        id: "timing-impact".into(),
        principal: Principal::Agent,
        state: Some(project.clone()),
        method: "change.dryRun".into(),
        params: json!({
            "expectedRevision": project.revision,
            "command": Command::SetTempoMap { tempo_map: tempo_map() }
        }),
    });
    let Response::Success(response) = response else {
        panic!("timing map dry-run failed")
    };
    assert_eq!(
        response.result["affectedInstances"]
            .as_array()
            .map(Vec::len),
        Some(project.instances.len())
    );
}

#[test]
fn timing_map_apply_retains_revision_and_authorization_guards() {
    let project = create_project(None, false);
    let stale = apply_change(
        &project,
        Principal::Human,
        ChangeRequest {
            expected_revision: "stale".into(),
            command: Command::SetTempoMap {
                tempo_map: tempo_map(),
            },
        },
    );
    assert!(matches!(stale, Err(CoreError::StaleRevision { .. })));

    let denied = apply_change(
        &project,
        Principal::Agent,
        ChangeRequest {
            expected_revision: project.revision.clone(),
            command: Command::SetMeterMap {
                meter_map: meter_map(),
            },
        },
    );
    assert!(matches!(denied, Err(CoreError::PermissionDenied)));
}
