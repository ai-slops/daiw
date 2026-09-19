use daiw_core::{
    Asset, AssetId, AudioRegion, ChangeRequest, Command, Principal, RegionId, Tick, TrackId,
    apply_change, create_project,
};
use std::fmt::Debug;

fn success<T, E: Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("unexpected error: {error:?}"),
    }
}

#[test]
fn repeated_asset_placement_reuses_canonical_asset() {
    // Given
    let project = create_project(Some("Asset reuse"), true);
    let asset = Asset {
        id: AssetId::from("asset"),
        name: "tone.wav".into(),
        hash: "hash".into(),
        frames: 44_100,
        sample_rate: 44_100,
        channels: 1,
    };
    let region = |id: &str, start: i64| AudioRegion {
        id: RegionId::from(id),
        track_id: TrackId::from("track-fx"),
        asset_id: asset.id.clone(),
        start: Tick::from_ticks(start),
        duration: Tick::from_ticks(960),
        gain: 0.0,
        source_offset: 0.0,
    };
    let first = success(apply_change(
        &project,
        Principal::Human,
        ChangeRequest {
            expected_revision: project.revision.clone(),
            command: Command::AddAudioRegion {
                asset: asset.clone(),
                region: region("one", 0),
            },
        },
    ));

    // When
    let second = success(apply_change(
        &first,
        Principal::Human,
        ChangeRequest {
            expected_revision: first.revision.clone(),
            command: Command::AddAudioRegion {
                asset: asset.clone(),
                region: region("two", 960),
            },
        },
    ));

    // Then
    assert_eq!(second.assets.len(), 1);
    assert_eq!(second.regions.len(), 2);
}
