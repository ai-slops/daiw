use std::collections::BTreeMap;

use daiw_audio::{
    AudioRegion, CompiledDspProgram, DspExecutor, DspProcessError, DspProcessState, Instrument,
    PcmAsset, ResidentDspSources, Snapshot, Track,
};

#[test]
fn fault_after_committed_chunk_zeros_whole_call_and_preserves_cursor() {
    // Given
    let snapshot = Snapshot {
        revision: "late-fault".into(),
        tempo: 60.0,
        tempo_map: None,
        length: 4_097.0 / 48_000.0,
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
            duration: 4_097.0 / 48_000.0,
            gain: 24.0,
            source_offset: 0.0,
        }],
    };
    let mut samples = vec![0.25; 4_097];
    samples[4_096] = f32::MAX;
    let assets = BTreeMap::from([(
        "asset".into(),
        PcmAsset {
            sample_rate: 48_000,
            channels: 1,
            samples,
        },
    )]);
    let sources = ResidentDspSources::new(&assets);
    let program = CompiledDspProgram::compile(&snapshot, &sources, 48_000)
        .unwrap_or_else(|error| panic!("compile failed: {error}"));
    let mut executor = DspExecutor::prepare(program, sources)
        .unwrap_or_else(|error| panic!("prepare failed: {error}"));
    let mut output = [[7.0; 2]; 4_097];

    // When
    let first = executor.process(&mut output);
    let first_cursor = executor.cursor();
    let first_zeroed = output.iter().flatten().all(|sample| *sample == 0.0);
    let repeated = executor.process(&mut output);
    let repeated_cursor = executor.cursor();
    executor.reset();
    let reset_cursor = executor.cursor();
    let reset_state = executor.state();
    let reset = executor.process(&mut output);

    // Then
    let expected = Err(DspProcessError::NonFiniteTrack { track: 0 });
    assert_eq!(first, expected);
    assert!(first_zeroed);
    assert_eq!(first_cursor, 4_096);
    assert_eq!(repeated, expected);
    assert_eq!(repeated_cursor, 4_096);
    assert_eq!(reset_cursor, 0);
    assert_eq!(reset_state, DspProcessState::Running);
    assert_eq!(reset, expected);
    assert!(output.iter().flatten().all(|sample| *sample == 0.0));
    assert_eq!(executor.cursor(), 4_096);
    assert_eq!(executor.state(), DspProcessState::Faulted);
}
