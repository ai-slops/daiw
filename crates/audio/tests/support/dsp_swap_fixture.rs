use std::collections::BTreeMap;

use daiw_audio::{
    AudioRegion, CompiledDspProgram, DspExecutor, Instrument, PcmAsset, ResidentDspSources,
    Snapshot, Track,
};

pub const RATE: u32 = 48_000;

pub fn snapshot(revision: &str, frames: usize) -> Snapshot {
    Snapshot {
        revision: revision.into(),
        tempo: 60.0,
        tempo_map: None,
        length: frames as f64 / f64::from(RATE),
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
            duration: frames as f64 / f64::from(RATE),
            gain: 0.0,
            source_offset: 0.0,
        }],
    }
}

pub fn assets(sample: f32, frames: usize) -> BTreeMap<String, PcmAsset> {
    BTreeMap::from([(
        "asset".into(),
        PcmAsset {
            sample_rate: RATE,
            channels: 1,
            samples: vec![sample; frames],
        },
    )])
}

pub fn executor<'a>(
    revision: &str,
    assets: &'a BTreeMap<String, PcmAsset>,
    frames: usize,
) -> DspExecutor<'a> {
    executor_at_rate(revision, assets, frames, RATE)
}

pub fn executor_at_rate<'a>(
    revision: &str,
    assets: &'a BTreeMap<String, PcmAsset>,
    frames: usize,
    rate: u32,
) -> DspExecutor<'a> {
    let snapshot = snapshot(revision, frames);
    let sources = ResidentDspSources::new(assets);
    let program = CompiledDspProgram::compile(&snapshot, &sources, rate)
        .unwrap_or_else(|error| panic!("compile failed: {error}"));
    DspExecutor::prepare(program, sources).unwrap_or_else(|error| panic!("prepare failed: {error}"))
}

pub fn fault_executor<'a>(assets: &'a BTreeMap<String, PcmAsset>) -> DspExecutor<'a> {
    let mut snapshot = snapshot("fault", 2);
    snapshot.regions[0].gain = 24.0;
    let sources = ResidentDspSources::new(assets);
    let program = CompiledDspProgram::compile(&snapshot, &sources, RATE)
        .unwrap_or_else(|error| panic!("compile failed: {error}"));
    DspExecutor::prepare(program, sources).unwrap_or_else(|error| panic!("prepare failed: {error}"))
}
