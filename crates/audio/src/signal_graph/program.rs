use crate::render_clock::RenderClock;
use crate::source::{SourceMetadata, SourceMetadataProvider};
use crate::validate::validate_snapshot;
use crate::{AudioError, Snapshot};

use super::events::{PreparedNote, PreparedRegion, PreparedTrack};
use super::graph::SignalGraph;
#[cfg(test)]
use super::graph::{Edge, Node};

pub const MAX_DSP_PREPARED_EVENTS: usize = 4096;

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
pub(crate) struct ProgramProvenance {
    pub(crate) snapshot_revision: String,
    pub(crate) sample_rate: u32,
    pub(crate) total_frames: usize,
}

pub struct CompiledDspProgram {
    pub(crate) graph: SignalGraph,
    pub(crate) instructions: Vec<Instruction>,
    pub(crate) buffer_slots: [BufferSlot; 2],
    pub(crate) latency_frames: usize,
    pub(crate) provenance: ProgramProvenance,
    pub(crate) tracks: Vec<PreparedTrack>,
    pub(crate) notes: Vec<PreparedNote>,
    pub(crate) regions: Vec<PreparedRegion>,
    pub(crate) source_ids: Vec<String>,
    pub(crate) source_metadata: Vec<SourceMetadata>,
}

impl CompiledDspProgram {
    /// Compiles an owned immutable program without retaining the snapshot or source samples.
    pub fn compile(
        snapshot: &Snapshot,
        sources: &impl SourceMetadataProvider,
        sample_rate: u32,
    ) -> Result<Self, AudioError> {
        validate_snapshot(snapshot, sample_rate)?;
        let clock = RenderClock::new(snapshot, sample_rate)?;
        super::compile::compile_public(snapshot, sources, &clock)
    }

    #[must_use]
    pub const fn sample_rate(&self) -> u32 {
        self.provenance.sample_rate
    }

    #[must_use]
    pub const fn total_frames(&self) -> usize {
        self.provenance.total_frames
    }

    #[must_use]
    pub fn revision(&self) -> &str {
        &self.provenance.snapshot_revision
    }

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
    pub(crate) const fn provenance(&self) -> &ProgramProvenance {
        &self.provenance
    }

    #[cfg(test)]
    pub(crate) fn active_track_ids(&self) -> Vec<&str> {
        self.tracks
            .iter()
            .map(|track| track.diagnostic_id.as_str())
            .collect()
    }
}
