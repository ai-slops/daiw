use std::cell::Cell;
use std::collections::BTreeMap;

use daiw_audio::{
    AudioError, AudioRegion, CompiledDspProgram, DspExecutor, DspProcessState, EffectiveNote,
    Instrument, MAX_DSP_PREPARED_EVENTS, Origin, PcmAsset, ResidentDspSources, Snapshot,
    SourceMetadata, SourceMetadataProvider, Track,
};

fn track() -> Track {
    Track {
        id: "duplicate".into(),
        name: "track".into(),
        role: "music".into(),
        color: "#fff".into(),
        instrument: Instrument::Synth,
        gain: 0.0,
        pan: 0.0,
        mute: false,
        solo: false,
        cutoff: 12_000.0,
    }
}

fn snapshot(notes: usize, tracks: usize, frames: usize) -> Snapshot {
    Snapshot {
        revision: "limit".into(),
        tempo: 60.0,
        tempo_map: None,
        length: frames as f64 / 48_000.0,
        tracks: (0..tracks).map(|_| track()).collect(),
        notes: (0..notes)
            .map(|index| EffectiveNote {
                id: format!("note-{index}"),
                start: 0.0,
                duration: 1.0 / 48_000.0,
                pitch: 60.0,
                velocity: 1.0,
                instance_id: "instance".into(),
                track_id: "duplicate".into(),
                origin: Origin {
                    definition_id: "definition".into(),
                    note_id: format!("origin-{index}"),
                    modifier_ids: Vec::new(),
                },
            })
            .collect(),
        regions: Vec::new(),
    }
}

#[test]
fn public_event_limit_counts_duplicate_track_expansion() {
    // Given
    let assets = BTreeMap::new();
    let sources = ResidentDspSources::new(&assets);
    let exact = snapshot(MAX_DSP_PREPARED_EVENTS / 2, 2, 1);
    let above = snapshot(MAX_DSP_PREPARED_EVENTS / 2 + 1, 2, 1);

    // When
    let exact_result = CompiledDspProgram::compile(&exact, &sources, 48_000);
    let above_result = CompiledDspProgram::compile(&above, &sources, 48_000);

    // Then
    assert!(exact_result.is_ok());
    assert!(matches!(
        above_result,
        Err(AudioError::TooManyPreparedEvents {
            limit: MAX_DSP_PREPARED_EVENTS,
            actual
        }) if actual == MAX_DSP_PREPARED_EVENTS + 2
    ));
}

#[test]
fn zero_frame_program_bypasses_track_and_event_limits() {
    // Given
    let assets = BTreeMap::new();
    let sources = ResidentDspSources::new(&assets);
    let value = snapshot(MAX_DSP_PREPARED_EVENTS + 1, 257, 0);

    // When
    let program = CompiledDspProgram::compile(&value, &sources, 48_000);

    // Then
    assert!(matches!(program, Ok(program) if program.total_frames() == 0));
}

#[test]
fn zero_frame_executor_is_ended_before_process_and_after_reset() {
    // Given
    let assets = BTreeMap::new();
    let sources = ResidentDspSources::new(&assets);
    let program = CompiledDspProgram::compile(&snapshot(0, 257, 0), &sources, 48_000)
        .unwrap_or_else(|error| panic!("compile failed: {error}"));
    let mut executor = DspExecutor::prepare(program, sources)
        .unwrap_or_else(|error| panic!("prepare failed: {error}"));
    let mut empty = [];

    // When
    let before = executor.state();
    let report = executor
        .process(&mut empty)
        .unwrap_or_else(|error| panic!("process failed: {error}"));
    executor.reset();

    // Then
    assert_eq!(before, DspProcessState::Ended);
    assert_eq!(report.state, DspProcessState::Ended);
    assert_eq!(report.frames, 0);
    assert_eq!(executor.state(), DspProcessState::Ended);
}

struct ChangingMetadata {
    calls: Cell<usize>,
    first: SourceMetadata,
    later: SourceMetadata,
    first_phase_calls: usize,
}

impl SourceMetadataProvider for ChangingMetadata {
    fn metadata(&self, _asset_id: &str) -> Result<SourceMetadata, AudioError> {
        let call = self.calls.get();
        self.calls.set(call + 1);
        if call == 0 || call >= self.first_phase_calls {
            Ok(self.first)
        } else {
            Ok(self.later)
        }
    }
}

fn region_snapshot(regions: usize) -> Snapshot {
    let mut value = snapshot(0, 1, 1);
    value.tracks[0].instrument = Instrument::Sample;
    value.regions = (0..regions)
        .map(|index| AudioRegion {
            id: format!("region-{index}"),
            track_id: "duplicate".into(),
            asset_id: "asset".into(),
            start: 0.0,
            duration: 1.0 / 48_000.0,
            gain: 0.0,
            source_offset: 0.0,
        })
        .collect();
    value
}

#[test]
fn public_compile_caches_metadata_before_enforcing_event_limit() {
    // Given
    let provider = ChangingMetadata {
        calls: Cell::new(0),
        first: SourceMetadata {
            sample_rate: 48_000,
            channels: 1,
            frames: 1,
        },
        later: SourceMetadata {
            sample_rate: 48_000,
            channels: 1,
            frames: 0,
        },
        first_phase_calls: MAX_DSP_PREPARED_EVENTS + 1,
    };

    // When
    let result = CompiledDspProgram::compile(
        &region_snapshot(MAX_DSP_PREPARED_EVENTS + 1),
        &provider,
        48_000,
    );

    // Then
    assert!(matches!(
        result,
        Err(AudioError::TooManyPreparedEvents { .. })
    ));
    assert_eq!(provider.calls.get(), 1);
}

#[test]
fn repeated_regions_share_one_stable_metadata_binding() {
    // Given
    let expected = SourceMetadata {
        sample_rate: 48_000,
        channels: 1,
        frames: 2,
    };
    let provider = ChangingMetadata {
        calls: Cell::new(0),
        first: expected,
        later: SourceMetadata {
            sample_rate: 24_000,
            channels: 2,
            frames: 1,
        },
        first_phase_calls: usize::MAX,
    };
    let assets = BTreeMap::from([(
        "asset".into(),
        PcmAsset {
            sample_rate: 48_000,
            channels: 1,
            samples: vec![0.25; 2],
        },
    )]);

    // When
    let program = CompiledDspProgram::compile(&region_snapshot(2), &provider, 48_000)
        .unwrap_or_else(|error| panic!("compile failed: {error}"));
    let prepared = DspExecutor::prepare(program, ResidentDspSources::new(&assets));

    // Then
    assert!(prepared.is_ok());
    assert_eq!(provider.calls.get(), 1);
}
