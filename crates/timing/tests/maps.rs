use daiw_timing::{Curve, MeterMap, MeterPoint, TempoMap, TempoPoint, Tick};
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

#[test]
fn tempo_map_rejects_invalid_boundaries() {
    let invalid = [
        serde_json::json!({"points": []}),
        serde_json::json!({"points": [{"at": 1, "bpm": 120, "curve": "step"}]}),
        serde_json::json!({"points": [
            {"at": 0, "bpm": 120, "curve": "step"},
            {"at": 0, "bpm": 140, "curve": "step"}
        ]}),
        serde_json::json!({"points": [{"at": 0, "bpm": 19.99, "curve": "step"}]}),
        serde_json::json!({"points": [{"at": 0, "bpm": 400.01, "curve": "step"}]}),
        serde_json::json!({"points": [{"at": 0, "bpm": 120, "curve": "linear"}]}),
    ];
    for value in invalid {
        assert!(serde_json::from_value::<TempoMap>(value).is_err());
    }
}

#[test]
fn tempo_map_supports_stable_linear_integral_and_inverse() {
    let map = must(TempoMap::new(vec![
        TempoPoint::new(beat(0.0), 120.0, Curve::Linear),
        TempoPoint::new(beat(4.0), 180.0, Curve::Step),
    ]));

    let seconds = must(map.seconds_at(beat(4.0)));
    let expected = 4.0 * (180.0_f64 / 120.0).ln();
    assert!((seconds - expected).abs() < 1.0e-12);
    assert_eq!(must(map.beat_at(seconds)), beat(4.0));
    assert_eq!(
        must(map.beat_at(must(map.seconds_at(beat(2.375))))),
        beat(2.375)
    );
}

#[test]
fn equal_linear_tempos_use_the_constant_limit() {
    let map = must(TempoMap::new(vec![
        TempoPoint::new(beat(0.0), 120.0, Curve::Linear),
        TempoPoint::new(beat(4.0), 120.0, Curve::Step),
    ]));

    assert_eq!(must(map.seconds_at(beat(4.0))), 2.0);
    assert_eq!(must(map.beat_at(2.0)), beat(4.0));
}

#[test]
fn inverse_rejects_nonfinite_negative_and_out_of_timeline_results() {
    let map = must(TempoMap::new(vec![TempoPoint::new(
        beat(0.0),
        120.0,
        Curve::Step,
    )]));

    for seconds in [f64::NAN, f64::NEG_INFINITY, f64::INFINITY, -1.0, f64::MAX] {
        assert!(map.beat_at(seconds).is_err());
    }
}

#[test]
fn forward_conversion_rejects_beats_outside_the_map_domain() {
    let map = must(TempoMap::new(vec![TempoPoint::new(
        beat(0.0),
        120.0,
        Curve::Step,
    )]));

    assert!(map.seconds_at(Tick::from_ticks(-1)).is_err());
    assert!(map.seconds_at(Tick::from_ticks(i64::MIN)).is_err());
    assert!(map.seconds_at(Tick::from_ticks(i64::MAX)).is_err());
    assert!(
        map.seconds_at(Tick::from_ticks(daiw_timing::MAX_TICKS + 1))
            .is_err()
    );
}

#[test]
fn meter_map_requires_exact_previous_bar_boundaries() {
    let valid = MeterMap::new(vec![
        MeterPoint::new(beat(0.0), 7, 8),
        MeterPoint::new(beat(3.5), 3, 4),
        MeterPoint::new(beat(6.5), 4, 4),
    ]);
    assert!(valid.is_ok());

    let invalid = MeterMap::new(vec![
        MeterPoint::new(beat(0.0), 7, 8),
        MeterPoint::new(beat(3.0), 4, 4),
    ]);
    assert!(invalid.is_err());
}

#[test]
fn maps_enforce_point_count_and_positive_meter_values() {
    let too_many = (0_i64..=1024)
        .map(|index| TempoPoint::new(Tick::from_ticks(index), 120.0, Curve::Step))
        .collect();
    assert!(TempoMap::new(too_many).is_err());
    assert!(MeterMap::new(vec![MeterPoint::new(beat(0.0), 0, 4)]).is_err());
    assert!(MeterMap::new(vec![MeterPoint::new(beat(0.0), 4, 0)]).is_err());
}

#[test]
fn maps_reject_programmatic_points_outside_the_timeline() {
    for ticks in [daiw_timing::MAX_TICKS + 1, i64::MAX] {
        let beyond = Tick::from_ticks(ticks);
        assert!(
            TempoMap::new(vec![
                TempoPoint::new(beat(0.0), 120.0, Curve::Step),
                TempoPoint::new(beyond, 120.0, Curve::Step),
            ])
            .is_err()
        );
        assert!(
            MeterMap::new(vec![
                MeterPoint::new(beat(0.0), 4, 4),
                MeterPoint::new(beyond, 4, 4),
            ])
            .is_err()
        );
    }
}
