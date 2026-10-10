use crate::render_clock::RenderClock;
use crate::source::SourceBank;
use crate::{AudioError, Snapshot};

use super::events::{
    EventRanges, PreparedNote, PreparedRegion, PreparedTrack, prepare_note, prepare_region,
    prepare_track,
};
use super::graph::{Edge, Node, NodeId, NodeKind, NodeProvenance, SignalGraph, TrackStage};
use super::latency::compile_latency;
use super::schedule::compile_schedule;

pub(crate) const MAX_ACTIVE_TRACKS: usize = 256;
pub(crate) const MAX_NODES: usize = 1 + 3 * MAX_ACTIVE_TRACKS;
pub(crate) const MAX_EDGES: usize = 3 * MAX_ACTIVE_TRACKS;
pub(super) const MAX_INSTRUCTIONS: usize = 2 + 5 * MAX_ACTIVE_TRACKS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BufferSlot {
    Master,
    Track,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Instruction {
    ClearMaster,
    ClearTrack { track: usize },
    RenderTrackSource { track: usize },
    LowPass { track: usize },
    ValidateTrack { track: usize },
    GainPanIntoMaster { track: usize },
    ValidateMaster,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ProgramProvenance<'a> {
    pub(crate) snapshot_revision: &'a str,
    pub(crate) sample_rate: u32,
    pub(crate) total_frames: usize,
}

pub(crate) struct CompiledSignalProgram<'a> {
    pub(crate) graph: SignalGraph,
    pub(crate) instructions: Vec<Instruction>,
    pub(crate) buffer_slots: [BufferSlot; 2],
    pub(crate) latency_frames: usize,
    pub(crate) provenance: ProgramProvenance<'a>,
    pub(crate) tracks: Vec<PreparedTrack<'a>>,
    pub(crate) notes: Vec<PreparedNote<'a>>,
    pub(crate) regions: Vec<PreparedRegion<'a>>,
}

impl CompiledSignalProgram<'_> {
    pub(crate) fn instructions(&self) -> &[Instruction] {
        &self.instructions
    }

    pub(crate) const fn latency_frames(&self) -> usize {
        self.latency_frames
    }

    pub(crate) fn track_count(&self) -> usize {
        (self.graph.nodes.len() - 1) / 3
    }

    #[cfg(test)]
    pub(crate) fn nodes(&self) -> &[Node] {
        &self.graph.nodes
    }

    #[cfg(test)]
    pub(crate) fn edges(&self) -> &[Edge] {
        &self.graph.edges
    }

    pub(crate) const fn buffer_slots(&self) -> &[BufferSlot; 2] {
        &self.buffer_slots
    }

    #[cfg(test)]
    pub(crate) const fn provenance(&self) -> &ProgramProvenance<'_> {
        &self.provenance
    }

    #[cfg(test)]
    pub(crate) fn active_track_ids(&self) -> Vec<&str> {
        self.tracks
            .iter()
            .map(|track| track.track.id.as_str())
            .collect()
    }
}

pub(crate) fn compile<'a>(
    snapshot: &'a Snapshot,
    sources: &impl SourceBank,
    clock: &RenderClock,
) -> Result<CompiledSignalProgram<'a>, AudioError> {
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
    compile_bounded(
        snapshot,
        sources,
        clock,
        soloed,
        CompileSizes {
            active_count,
            node_count,
            edge_count,
        },
    )
}

struct CompileSizes {
    active_count: usize,
    node_count: usize,
    edge_count: usize,
}

fn compile_empty<'a>(
    snapshot: &'a Snapshot,
    clock: &RenderClock,
) -> Result<CompiledSignalProgram<'a>, AudioError> {
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
    Ok(CompiledSignalProgram {
        graph,
        instructions,
        buffer_slots: [BufferSlot::Master, BufferSlot::Track],
        latency_frames,
        provenance: ProgramProvenance {
            snapshot_revision: &snapshot.revision,
            sample_rate: clock.sample_rate,
            total_frames: clock.total_frames,
        },
        tracks: Vec::new(),
        notes: Vec::new(),
        regions: Vec::new(),
    })
}

fn compile_bounded<'a>(
    snapshot: &'a Snapshot,
    sources: &impl SourceBank,
    clock: &RenderClock,
    soloed: bool,
    sizes: CompileSizes,
) -> Result<CompiledSignalProgram<'a>, AudioError> {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut tracks = Vec::new();
    let mut notes = Vec::new();
    let mut regions = Vec::new();
    nodes.try_reserve_exact(sizes.node_count)?;
    edges.try_reserve_exact(sizes.edge_count)?;
    tracks.try_reserve_exact(sizes.active_count)?;
    notes.try_reserve(snapshot.notes.len())?;
    regions.try_reserve(snapshot.regions.len())?;
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
            if let Some(region) = prepare_region(region, sources, clock)? {
                regions.push(region);
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
    Ok(CompiledSignalProgram {
        graph,
        instructions,
        buffer_slots: [BufferSlot::Master, BufferSlot::Track],
        latency_frames,
        provenance: ProgramProvenance {
            snapshot_revision: &snapshot.revision,
            sample_rate: clock.sample_rate,
            total_frames: clock.total_frames,
        },
        tracks,
        notes,
        regions,
    })
}

fn append_track_graph(
    nodes: &mut Vec<Node>,
    edges: &mut Vec<Edge>,
    track_id: String,
    ordinal: usize,
) {
    let source = NodeId(nodes.len());
    let low_pass = NodeId(source.0 + 1);
    let gain_pan = NodeId(source.0 + 2);
    for (id, kind, stage) in [
        (source, NodeKind::TrackSource, TrackStage::Source),
        (low_pass, NodeKind::OnePoleLowPass, TrackStage::LowPass),
        (gain_pan, NodeKind::GainPan, TrackStage::GainPan),
    ] {
        nodes.push(Node {
            id,
            kind,
            latency_frames: kind.intrinsic_latency_frames(),
            provenance: NodeProvenance::Track {
                ordinal,
                track_id: track_id.clone(),
                stage,
            },
        });
    }
    edges.extend([
        Edge::new(source, low_pass, 0),
        Edge::new(low_pass, gain_pan, 0),
        Edge::new(gain_pan, NodeId(0), ordinal),
    ]);
}
