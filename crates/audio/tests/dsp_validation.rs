use std::collections::BTreeMap;

use daiw_audio::{
    AudioError, AudioRegion, CompiledDspProgram, DspExecutor, DspProcessError, DspProcessState,
    Instrument, PcmAsset, ResidentDspSources, Snapshot, SourceMetadata, SourceMetadataProvider,
    Track,
};

#[path = "support/must.rs"]
mod support;
use support::must;

#[derive(Clone, Copy)]
struct Metadata(SourceMetadata);

impl SourceMetadataProvider for Metadata {
    fn metadata(&self, _asset_id: &str) -> Result<SourceMetadata, AudioError> {
        Ok(self.0)
    }
}

fn sample_snapshot(gain: f64) -> Snapshot {
    Snapshot {
        revision: "validation".into(),
        tempo: 60.0,
        tempo_map: None,
        length: 2.0 / 48_000.0,
        tracks: vec![Track {
            id: "sample".into(),
            name: "sample".into(),
            role: "music".into(),
            color: "#fff".into(),
            instrument: Instrument::Sample,
            gain,
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
    }
}

fn compile_with(metadata: SourceMetadata, gain: f64) -> CompiledDspProgram {
    must(CompiledDspProgram::compile(
        &sample_snapshot(gain),
        &Metadata(metadata),
        48_000,
    ))
}

#[test]
fn preparation_rejects_metadata_channels_layout_and_non_finite_samples() {
    // Given
    let expected = SourceMetadata {
        sample_rate: 48_000,
        channels: 1,
        frames: 2,
    };
    let mismatch_assets = BTreeMap::from([(
        "asset".into(),
        PcmAsset {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.0; 2],
        },
    )]);
    let channels_assets = BTreeMap::from([(
        "asset".into(),
        PcmAsset {
            sample_rate: 48_000,
            channels: 9,
            samples: vec![0.0; 18],
        },
    )]);
    let layout_assets = BTreeMap::from([(
        "asset".into(),
        PcmAsset {
            sample_rate: 48_000,
            channels: 2,
            samples: vec![0.0; 3],
        },
    )]);
    let finite_assets = BTreeMap::from([(
        "asset".into(),
        PcmAsset {
            sample_rate: 48_000,
            channels: 1,
            samples: vec![0.0, f32::NAN],
        },
    )]);

    // When
    let mismatch = DspExecutor::prepare(
        compile_with(expected, 0.0),
        ResidentDspSources::new(&mismatch_assets),
    );
    let channels = DspExecutor::prepare(
        compile_with(
            SourceMetadata {
                channels: 9,
                ..expected
            },
            0.0,
        ),
        ResidentDspSources::new(&channels_assets),
    );
    let layout = DspExecutor::prepare(
        compile_with(
            SourceMetadata {
                channels: 2,
                frames: 1,
                ..expected
            },
            0.0,
        ),
        ResidentDspSources::new(&layout_assets),
    );
    let finite = DspExecutor::prepare(
        compile_with(expected, 0.0),
        ResidentDspSources::new(&finite_assets),
    );

    // Then
    assert!(matches!(mismatch, Err(AudioError::PcmMetadataMismatch)));
    assert!(matches!(
        channels,
        Err(AudioError::UnsupportedPcmChannels { channels: 9 })
    ));
    assert!(matches!(layout, Err(AudioError::InvalidAssetLayout { .. })));
    assert!(matches!(
        finite,
        Err(AudioError::InvalidValue {
            field: "samples",
            ..
        })
    ));
}

#[test]
fn process_fault_is_sticky_zeros_whole_call_and_reset_restarts() {
    // Given
    let metadata = SourceMetadata {
        sample_rate: 48_000,
        channels: 1,
        frames: 2,
    };
    let assets = BTreeMap::from([(
        "asset".into(),
        PcmAsset {
            sample_rate: 48_000,
            channels: 1,
            samples: vec![f32::MAX; 2],
        },
    )]);
    let mut executor = must(DspExecutor::prepare(
        compile_with(metadata, 0.0),
        ResidentDspSources::new(&assets),
    ));
    let mut output = [[7.0; 2]; 4_097];

    // When
    let first = executor.process(&mut output);
    let repeated = executor.process(&mut output);
    executor.reset();
    let after_reset = executor.process(&mut output);

    // Then
    assert!(matches!(
        first,
        Err(DspProcessError::NonFiniteTrack { track: 0 })
    ));
    assert_eq!(repeated, first);
    assert_eq!(after_reset, first);
    assert!(output.iter().flatten().all(|sample| *sample == 0.0));
    assert_eq!(executor.state(), DspProcessState::Faulted);
    assert_eq!(executor.cursor(), 0);
}

#[test]
fn finite_tracks_that_overflow_the_master_report_master_fault() {
    // Given
    let metadata = SourceMetadata {
        sample_rate: 48_000,
        channels: 1,
        frames: 2,
    };
    let mut value = sample_snapshot(770.0);
    value.tracks.push(value.tracks[0].clone());
    let assets = BTreeMap::from([(
        "asset".into(),
        PcmAsset {
            sample_rate: 48_000,
            channels: 1,
            samples: vec![1.0; 2],
        },
    )]);
    let program = must(CompiledDspProgram::compile(
        &value,
        &Metadata(metadata),
        48_000,
    ));
    let mut executor = must(DspExecutor::prepare(
        program,
        ResidentDspSources::new(&assets),
    ));
    let mut output = [[3.0; 2]; 2];

    // When
    let result = executor.process(&mut output);

    // Then
    assert_eq!(result, Err(DspProcessError::NonFiniteMaster));
    assert_eq!(output, [[0.0; 2]; 2]);
    assert_eq!(executor.state(), DspProcessState::Faulted);
}
