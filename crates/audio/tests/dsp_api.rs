use std::collections::BTreeMap;

use daiw_audio::{
    AudioRegion, CompiledDspProgram, DspExecutor, DspProcessState, EffectiveNote, Instrument,
    Origin, PcmAsset, RenderRequest, ResidentDspSources, Snapshot, Track, render_with_sample_rate,
};

#[path = "support/must.rs"]
mod support;
use support::must;

fn track() -> Track {
    Track {
        id: "synth".into(),
        name: "synth".into(),
        role: "music".into(),
        color: "#fff".into(),
        instrument: Instrument::Synth,
        gain: 0.0,
        pan: 0.0,
        mute: false,
        solo: false,
        cutoff: 8_000.0,
    }
}

fn snapshot(frames: usize) -> Snapshot {
    Snapshot {
        revision: "dsp-r1".into(),
        tempo: 60.0,
        tempo_map: None,
        length: frames as f64 / 48_000.0,
        tracks: vec![track()],
        notes: vec![EffectiveNote {
            id: "note".into(),
            start: 0.0,
            duration: frames as f64 / 48_000.0,
            pitch: 69.0,
            velocity: 0.8,
            instance_id: "instance".into(),
            track_id: "synth".into(),
            origin: Origin {
                definition_id: "definition".into(),
                note_id: "source-note".into(),
                modifier_ids: Vec::new(),
            },
        }],
        regions: Vec::new(),
    }
}

#[test]
fn executor_streams_variable_blocks_then_resets_from_ended() {
    // Given
    let assets = BTreeMap::new();
    let sources = ResidentDspSources::new(&assets);
    let program = must(CompiledDspProgram::compile(&snapshot(5), &sources, 48_000));
    assert_eq!(program.sample_rate(), 48_000);
    assert_eq!(program.total_frames(), 5);
    assert_eq!(program.revision(), "dsp-r1");
    let mut executor = must(DspExecutor::prepare(program, sources));
    let mut empty = [];
    let mut first = [[9.0; 2]; 3];
    let mut second = [[9.0; 2]; 4];

    // When
    let empty_report = must(executor.process(&mut empty));
    let first_report = must(executor.process(&mut first));
    let second_report = must(executor.process(&mut second));

    // Then
    assert_eq!(empty_report.frames, 0);
    assert_eq!(empty_report.state, DspProcessState::Running);
    assert_eq!(first_report.state, DspProcessState::Running);
    assert_eq!(first_report.frames, 3);
    assert_eq!(second_report.state, DspProcessState::Ended);
    assert_eq!(second_report.frames, 2);
    assert_eq!(second[2..], [[0.0; 2]; 2]);
    assert_eq!(executor.cursor(), 5);
    assert_eq!(executor.state(), DspProcessState::Ended);
    executor.reset();
    assert_eq!(executor.cursor(), 0);
    assert_eq!(executor.state(), DspProcessState::Running);
}

fn parity_request() -> RenderRequest {
    let mut value = snapshot(4_101);
    value.tracks.push(Track {
        instrument: Instrument::Sample,
        id: "sample".into(),
        name: "sample".into(),
        ..track()
    });
    value.regions.push(AudioRegion {
        id: "region".into(),
        track_id: "sample".into(),
        asset_id: "pcm".into(),
        start: 1.0 / 48_000.0,
        duration: 4_100.0 / 48_000.0,
        gain: -2.0,
        source_offset: 0.0,
    });
    let samples = (0..4_101)
        .flat_map(|frame| {
            let sample = (frame % 97) as f32 / 97.0;
            [sample, -sample]
        })
        .collect();
    RenderRequest {
        snapshot: value,
        assets: BTreeMap::from([(
            "pcm".into(),
            PcmAsset {
                sample_rate: 32_000,
                channels: 2,
                samples,
            },
        )]),
    }
}

fn execute_in_blocks(request: &RenderRequest, block: usize) -> Vec<f32> {
    let sources = ResidentDspSources::new(&request.assets);
    let program = must(CompiledDspProgram::compile(
        &request.snapshot,
        &sources,
        48_000,
    ));
    let frames = program.total_frames();
    let mut executor = must(DspExecutor::prepare(program, sources));
    let mut output = Vec::with_capacity(frames * 2);
    let mut scratch = vec![[0.0; 2]; block];
    while executor.state() != DspProcessState::Ended {
        let report = must(executor.process(&mut scratch));
        output.extend(scratch[..report.frames].iter().flatten().copied());
    }
    output
}

#[test]
fn executor_is_bit_identical_to_offline_for_all_callback_boundaries() {
    // Given
    let request = parity_request();
    let expected = must(render_with_sample_rate(&request, 48_000)).samples;

    // When
    let one = execute_in_blocks(&request, 1);
    let three = execute_in_blocks(&request, 3);
    let production = execute_in_blocks(&request, 4_096);
    let larger = execute_in_blocks(&request, 4_097);

    // Then
    assert_eq!(one, expected);
    assert_eq!(three, expected);
    assert_eq!(production, expected);
    assert_eq!(larger, expected);
}
