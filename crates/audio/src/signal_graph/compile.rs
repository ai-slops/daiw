use crate::render_clock::RenderClock;
use crate::source::{SourceMetadata, SourceMetadataProvider};
use crate::{AudioError, Snapshot};

use super::event_count::count_prepared_events;
use super::events::{EventRanges, prepare_note, prepare_region, prepare_track};
use super::graph::{Node, NodeId, NodeKind, NodeProvenance, SignalGraph};
use super::graph_build::append_track_graph;
use super::latency::compile_latency;
use super::metadata_cache::CachedMetadataProvider;
use super::program::{
    BufferSlot, CompiledDspProgram, Instruction, MAX_DSP_PREPARED_EVENTS, ProgramProvenance,
};
use super::schedule::compile_schedule;

pub(crate) const MAX_ACTIVE_TRACKS: usize = 256;
pub(crate) const MAX_NODES: usize = 1 + 3 * MAX_ACTIVE_TRACKS;
pub(crate) const MAX_EDGES: usize = 3 * MAX_ACTIVE_TRACKS;
pub(super) const MAX_INSTRUCTIONS: usize = 2 + 5 * MAX_ACTIVE_TRACKS;
pub(crate) fn compile(
    snapshot: &Snapshot,
    sources: &impl SourceMetadataProvider,
    clock: &RenderClock,
) -> Result<CompiledDspProgram, AudioError> {
    compile_with_event_limit(snapshot, sources, clock, None)
}

pub(super) fn compile_public(
    snapshot: &Snapshot,
    sources: &impl SourceMetadataProvider,
    clock: &RenderClock,
) -> Result<CompiledDspProgram, AudioError> {
    let cached = CachedMetadataProvider::new(sources);
    compile_with_event_limit(snapshot, &cached, clock, Some(MAX_DSP_PREPARED_EVENTS))
}

fn compile_with_event_limit(
    snapshot: &Snapshot,
    sources: &impl SourceMetadataProvider,
    clock: &RenderClock,
    event_limit: Option<usize>,
) -> Result<CompiledDspProgram, AudioError> {
    if clock.total_frames == 0 {
        return compile_empty(snapshot, clock);
    }
    let soloed = snapshot.tracks.iter().any(|track| track.solo);
    let active_count = snapshot
        .tracks
        .iter()
        .filter(|track| !track.mute && (!soloed || track.solo))
        .count();
    if active_count > MAX_ACTIVE_TRACKS {
        return Err(AudioError::TooManyActiveTracks {
            limit: MAX_ACTIVE_TRACKS,
            actual: active_count,
        });
    }
    let prepared_event_capacity = if let Some(limit) = event_limit {
        let actual = count_prepared_events(snapshot, sources, clock, soloed)?;
        if actual > limit {
            return Err(AudioError::TooManyPreparedEvents { limit, actual });
        }
        Some(actual)
    } else {
        None
    };
    let node_count = active_count
        .checked_mul(3)
        .and_then(|count| count.checked_add(1))
        .ok_or(AudioError::SignalGraphLimitExceeded {
            resource: "nodes",
            limit: MAX_NODES,
            actual: usize::MAX,
        })?;
    let edge_count = active_count
        .checked_mul(3)
        .ok_or(AudioError::SignalGraphLimitExceeded {
            resource: "edges",
            limit: MAX_EDGES,
            actual: usize::MAX,
        })?;
    let program = compile_bounded(
        snapshot,
        sources,
        clock,
        soloed,
        CompileSizes {
            active_count,
            node_count,
            edge_count,
            prepared_event_capacity,
        },
    )?;
    if prepared_event_capacity.is_some_and(|expected| {
        expected != program.notes.len().saturating_add(program.regions.len())
    }) {
        return Err(AudioError::InvalidSignalGraph {
            issue: "prepared event count changed during compilation",
        });
    }
    Ok(program)
}

struct CompileSizes {
    active_count: usize,
    node_count: usize,
    edge_count: usize,
    prepared_event_capacity: Option<usize>,
}

fn compile_empty(
    snapshot: &Snapshot,
    clock: &RenderClock,
) -> Result<CompiledDspProgram, AudioError> {
    let master_kind = NodeKind::Master;
    let mut nodes = Vec::new();
    nodes.try_reserve_exact(1)?;
    nodes.push(Node {
        id: NodeId(0),
        kind: master_kind,
        latency_frames: master_kind.intrinsic_latency_frames(),
        provenance: NodeProvenance::Master,
    });
    let graph = SignalGraph {
        nodes,
        edges: Vec::new(),
        master: NodeId(0),
    };
    graph.validate()?;
    let instructions = compile_schedule(&graph)?;
    let latency_frames = compile_latency(&graph)?;
    Ok(CompiledDspProgram {
        graph,
        instructions,
        buffer_slots: [BufferSlot::Master, BufferSlot::Track],
        latency_frames,
        provenance: ProgramProvenance {
            snapshot_revision: snapshot.revision.clone(),
            sample_rate: clock.sample_rate,
            total_frames: clock.total_frames,
        },
        tracks: Vec::new(),
        notes: Vec::new(),
        regions: Vec::new(),
        source_ids: Vec::new(),
        source_metadata: Vec::new(),
    })
}

fn compile_bounded(
    snapshot: &Snapshot,
    sources: &impl SourceMetadataProvider,
    clock: &RenderClock,
    soloed: bool,
    sizes: CompileSizes,
) -> Result<CompiledDspProgram, AudioError> {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut tracks = Vec::new();
    let mut notes = Vec::new();
    let mut regions = Vec::new();
    let mut source_ids = Vec::new();
    let mut source_metadata = Vec::new();
    nodes.try_reserve_exact(sizes.node_count)?;
    edges.try_reserve_exact(sizes.edge_count)?;
    tracks.try_reserve_exact(sizes.active_count)?;
    notes.try_reserve(
        sizes
            .prepared_event_capacity
            .map_or(snapshot.notes.len(), |count| {
                count.min(snapshot.notes.len())
            }),
    )?;
    regions.try_reserve(
        sizes
            .prepared_event_capacity
            .map_or(snapshot.regions.len(), |count| {
                count.min(snapshot.regions.len())
            }),
    )?;
    let master_kind = NodeKind::Master;
    nodes.push(Node {
        id: NodeId(0),
        kind: master_kind,
        latency_frames: master_kind.intrinsic_latency_frames(),
        provenance: NodeProvenance::Master,
    });
    for track in snapshot
        .tracks
        .iter()
        .filter(|track| !track.mute && (!soloed || track.solo))
    {
        let ordinal = tracks.len();
        let note_start = notes.len();
        for note in snapshot
            .notes
            .iter()
            .filter(|note| note.track_id == track.id)
        {
            if let Some(note) = prepare_note(note, track.instrument, clock)? {
                notes.push(note);
            }
        }
        let region_start = regions.len();
        for region in snapshot
            .regions
            .iter()
            .filter(|region| region.track_id == track.id)
        {
            let source_index = match source_ids.iter().position(|id| id == &region.asset_id) {
                Some(index) => index,
                None => source_ids.len(),
            };
            if let Some(prepared) = prepare_region(region, source_index, sources, clock)? {
                if source_index == source_ids.len() {
                    source_ids.push(region.asset_id.clone());
                    source_metadata.push(SourceMetadata {
                        sample_rate: prepared.source_rate,
                        channels: prepared.channels,
                        frames: prepared.source_start + prepared.source_frames,
                    });
                }
                regions.push(prepared);
            }
        }
        tracks.push(prepare_track(
            track,
            EventRanges {
                notes: note_start..notes.len(),
                regions: region_start..regions.len(),
            },
            clock,
        )?);
        append_track_graph(&mut nodes, &mut edges, track.id.clone(), ordinal);
    }
    let graph = SignalGraph {
        nodes,
        edges,
        master: NodeId(0),
    };
    graph.validate()?;
    let instructions = compile_schedule(&graph)?;
    let scheduled_tracks = instructions
        .iter()
        .filter(|instruction| matches!(instruction, Instruction::RenderTrackSource { .. }))
        .count();
    if scheduled_tracks != tracks.len() {
        return Err(AudioError::InvalidSignalGraph {
            issue: "schedule track count differs from prepared tracks",
        });
    }
    let latency_frames = compile_latency(&graph)?;
    Ok(CompiledDspProgram {
        graph,
        instructions,
        buffer_slots: [BufferSlot::Master, BufferSlot::Track],
        latency_frames,
        provenance: ProgramProvenance {
            snapshot_revision: snapshot.revision.clone(),
            sample_rate: clock.sample_rate,
            total_frames: clock.total_frames,
        },
        tracks,
        notes,
        regions,
        source_ids,
        source_metadata,
    })
}
