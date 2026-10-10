use std::collections::BTreeMap;

use super::{
    BufferSlot, Instruction, MAX_ACTIVE_TRACKS, MAX_EDGES, MAX_NODES, NodeKind, NodeProvenance,
    TrackStage, compile,
};
use crate::render_clock::RenderClock;
use crate::source::ResidentSourceBank;
use crate::{AudioError, Instrument, Snapshot, Track};

fn track(id: usize) -> Track {
    Track {
        id: format!("track-{id}"),
        name: format!("Track {id}"),
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

fn snapshot(track_count: usize) -> Snapshot {
    Snapshot {
        revision: "graph-revision".into(),
        tempo: 120.0,
        tempo_map: None,
        length: 1.0,
        tracks: (0..track_count).map(track).collect(),
        notes: vec![],
        regions: vec![],
    }
}

fn compile_snapshot(snapshot: &Snapshot) -> Result<super::CompiledSignalProgram<'_>, AudioError> {
    let clock = RenderClock::new(snapshot, 48_000)?;
    let assets = BTreeMap::new();
    let sources = ResidentSourceBank::new(&assets);
    compile(snapshot, &sources, &clock)
}

#[test]
fn program_has_fixed_topology_schedule_and_provenance_for_two_tracks() -> Result<(), AudioError> {
    // Given
    let snapshot = snapshot(2);

    // When
    let program = compile_snapshot(&snapshot)?;

    // Then
    assert_eq!(program.nodes().len(), 7);
    assert_eq!(program.edges().len(), 6);
    assert_eq!(
        program.buffer_slots(),
        &[BufferSlot::Master, BufferSlot::Track]
    );
    assert_eq!(program.latency_frames(), 0);
    assert_eq!(program.provenance().snapshot_revision, "graph-revision");
    assert_eq!(program.provenance().sample_rate, 48_000);
    assert_eq!(program.provenance().total_frames, 24_000);
    assert!(program.nodes().iter().all(|node| node.latency_frames == 0));
    assert_eq!(program.nodes()[0].kind, NodeKind::Master);
    for (ordinal, nodes) in program.nodes()[1..].chunks_exact(3).enumerate() {
        for (node, (kind, stage)) in nodes.iter().zip([
            (NodeKind::TrackSource, TrackStage::Source),
            (NodeKind::OnePoleLowPass, TrackStage::LowPass),
            (NodeKind::GainPan, TrackStage::GainPan),
        ]) {
            assert_eq!(node.kind, kind);
            assert_eq!(
                node.provenance,
                NodeProvenance::Track {
                    ordinal,
                    track_id: format!("track-{ordinal}"),
                    stage,
                }
            );
        }
    }
    assert_eq!(
        program.instructions(),
        &[
            Instruction::ClearMaster,
            Instruction::ClearTrack { track: 0 },
            Instruction::RenderTrackSource { track: 0 },
            Instruction::LowPass { track: 0 },
            Instruction::ValidateTrack { track: 0 },
            Instruction::GainPanIntoMaster { track: 0 },
            Instruction::ClearTrack { track: 1 },
            Instruction::RenderTrackSource { track: 1 },
            Instruction::LowPass { track: 1 },
            Instruction::ValidateTrack { track: 1 },
            Instruction::GainPanIntoMaster { track: 1 },
            Instruction::ValidateMaster,
        ]
    );
    Ok(())
}

#[test]
fn active_tracks_follow_snapshot_order_with_mute_and_solo() -> Result<(), AudioError> {
    // Given
    let mut snapshot = snapshot(4);
    snapshot.tracks[0].solo = true;
    snapshot.tracks[1].solo = true;
    snapshot.tracks[1].mute = true;
    snapshot.tracks[3].solo = true;

    // When
    let program = compile_snapshot(&snapshot)?;

    // Then
    assert_eq!(program.active_track_ids(), ["track-0", "track-3"]);
    Ok(())
}

#[test]
fn empty_snapshot_compiles_master_only_program() -> Result<(), AudioError> {
    // Given
    let snapshot = snapshot(0);

    // When
    let program = compile_snapshot(&snapshot)?;

    // Then
    assert_eq!(program.nodes().len(), 1);
    assert!(program.edges().is_empty());
    assert_eq!(
        program.instructions(),
        &[Instruction::ClearMaster, Instruction::ValidateMaster]
    );
    Ok(())
}

#[test]
fn compiler_accepts_maximum_active_tracks() -> Result<(), AudioError> {
    // Given
    let snapshot = snapshot(MAX_ACTIVE_TRACKS);

    // When
    let program = compile_snapshot(&snapshot)?;

    // Then
    assert_eq!(program.nodes().len(), MAX_NODES);
    assert_eq!(program.edges().len(), MAX_EDGES);
    Ok(())
}

#[test]
fn compiler_rejects_track_count_above_limit_before_allocating() -> Result<(), AudioError> {
    // Given
    let snapshot = snapshot(MAX_ACTIVE_TRACKS + 1);

    // When
    let result = compile_snapshot(&snapshot);

    // Then
    assert!(matches!(
        result,
        Err(AudioError::TooManyActiveTracks {
            limit: MAX_ACTIVE_TRACKS,
            actual
        }) if actual == MAX_ACTIVE_TRACKS + 1
    ));
    Ok(())
}

#[test]
fn zero_frame_program_skips_limits_and_derived_track_values() -> Result<(), AudioError> {
    // Given
    let mut snapshot = snapshot(MAX_ACTIVE_TRACKS + 1);
    snapshot.length = 0.0;
    for track in &mut snapshot.tracks {
        track.gain = f64::MAX;
        track.pan = f64::MAX;
        track.cutoff = f64::MAX;
    }

    // When
    let program = compile_snapshot(&snapshot)?;

    // Then
    assert_eq!(program.nodes().len(), 1);
    assert!(program.edges().is_empty());
    assert_eq!(program.track_count(), 0);
    assert_eq!(
        program.instructions(),
        &[Instruction::ClearMaster, Instruction::ValidateMaster]
    );
    Ok(())
}

#[test]
fn schedule_follows_master_ports_when_edge_storage_order_changes() -> Result<(), AudioError> {
    // Given
    let snapshot = snapshot(2);
    let mut program = compile_snapshot(&snapshot)?;
    program.graph.edges.reverse();
    program.graph.validate()?;

    // When
    let instructions = super::schedule::compile_schedule(&program.graph)?;

    // Then
    assert_eq!(instructions, program.instructions());
    Ok(())
}

#[test]
fn duplicate_track_ids_keep_distinct_ordinal_provenance() -> Result<(), AudioError> {
    // Given
    let mut snapshot = snapshot(2);
    snapshot.tracks[0].id = "duplicate".into();
    snapshot.tracks[1].id = "duplicate".into();

    // When
    let program = compile_snapshot(&snapshot)?;

    // Then
    assert_eq!(program.active_track_ids(), ["duplicate", "duplicate"]);
    for (ordinal, node) in [program.nodes()[1].clone(), program.nodes()[4].clone()]
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            node.provenance,
            NodeProvenance::Track {
                ordinal,
                track_id: "duplicate".into(),
                stage: TrackStage::Source,
            }
        );
    }
    Ok(())
}
