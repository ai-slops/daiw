mod compile;
mod event_count;
mod events;
mod graph;
mod graph_build;
mod latency;
mod metadata_cache;
mod program;
mod schedule;
mod validation;

pub(crate) use compile::compile;
#[cfg(test)]
pub(crate) use compile::{MAX_ACTIVE_TRACKS, MAX_EDGES, MAX_NODES};
pub(crate) use events::{PreparedNote, PreparedRegion};
#[cfg(test)]
pub(crate) use graph::{
    Edge, Node, NodeId, NodeKind, NodeProvenance, SignalGraph, StereoInput, TrackStage,
};
pub(crate) use program::{BufferSlot, Instruction};
pub use program::{CompiledDspProgram, MAX_DSP_PREPARED_EVENTS};

#[cfg(test)]
mod compile_tests;
#[cfg(test)]
mod graph_tests;
