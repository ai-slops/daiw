use std::alloc::System;
use std::collections::BTreeMap;

use daiw_audio::{
    AudioRegion, CompiledDspProgram, DspExecutor, Instrument, PcmAsset, ResidentDspSources,
    Snapshot, Track,
};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

fn request(sample: f32) -> (Snapshot, BTreeMap<String, PcmAsset>) {
    let snapshot = Snapshot {
        revision: "alloc".into(),
        tempo: 60.0,
        tempo_map: None,
        length: 2.0 / 48_000.0,
        tracks: vec![Track {
            id: "sample".into(),
            name: "sample".into(),
            role: "music".into(),
            color: "#fff".into(),
            instrument: Instrument::Sample,
            gain: 0.0,
            pan: 0.0,
            mute: false,
            solo: false,
            cutoff: 20_000.0,
        }],
        notes: Vec::new(),
        regions: vec![AudioRegion {
            id: "region".into(),
            track_id: "sample".into(),
            asset_id: "asset".into(),
            start: 0.0,
            duration: 2.0 / 48_000.0,
            gain: 24.0,
            source_offset: 0.0,
        }],
    };
    let assets = BTreeMap::from([(
        "asset".into(),
        PcmAsset {
            sample_rate: 48_000,
            channels: 1,
            samples: vec![sample; 2],
        },
    )]);
    (snapshot, assets)
}

fn executor<'a>(snapshot: &Snapshot, assets: &'a BTreeMap<String, PcmAsset>) -> DspExecutor<'a> {
    let sources = ResidentDspSources::new(assets);
    let program = CompiledDspProgram::compile(snapshot, &sources, 48_000)
        .unwrap_or_else(|error| panic!("compile failed: {error}"));
    DspExecutor::prepare(program, sources).unwrap_or_else(|error| panic!("prepare failed: {error}"))
}

fn late_fault_request() -> (Snapshot, BTreeMap<String, PcmAsset>) {
    let (mut snapshot, mut assets) = request(0.25);
    snapshot.length = 4_097.0 / 48_000.0;
    snapshot.regions[0].duration = snapshot.length;
    let samples = &mut assets
        .get_mut("asset")
        .unwrap_or_else(|| panic!("asset fixture missing"))
        .samples;
    samples.resize(4_097, 0.25);
    samples[4_096] = f32::MAX;
    (snapshot, assets)
}

#[test]
fn process_eof_fault_repeat_and_reset_do_not_allocate() {
    // Given
    let (safe_snapshot, safe_assets) = request(0.25);
    let (fault_snapshot, fault_assets) = late_fault_request();
    let mut safe = executor(&safe_snapshot, &safe_assets);
    let mut fault = executor(&fault_snapshot, &fault_assets);
    let mut output = [[0.0; 2]; 4_097];
    let region = Region::new(GLOBAL);

    // When
    let success = safe.process(&mut output);
    let eof = safe.process(&mut output);
    let first_fault = fault.process(&mut output);
    let repeated_fault = fault.process(&mut output);
    fault.reset();
    let reset_fault = fault.process(&mut output);

    // Then
    assert!(success.is_ok());
    assert!(eof.is_ok());
    assert!(first_fault.is_err());
    assert_eq!(repeated_fault, first_fault);
    assert_eq!(reset_fault, first_fault);
    assert_eq!(fault.cursor(), 4_096);
    let change = region.change();
    assert_eq!(change.allocations, 0);
    assert_eq!(change.deallocations, 0);
}
