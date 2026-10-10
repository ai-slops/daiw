mod compile;
mod events;
mod graph;
mod latency;
mod schedule;
mod validation;

pub(crate) use compile::{BufferSlot, CompiledSignalProgram, Instruction, compile};
#[cfg(test)]
pub(crate) use compile::{MAX_ACTIVE_TRACKS, MAX_EDGES, MAX_NODES};
pub(crate) use events::{PreparedNote, PreparedRegion};
#[cfg(test)]
pub(crate) use graph::{
    Edge, Node, NodeId, NodeKind, NodeProvenance, SignalGraph, StereoInput, TrackStage,
};

#[cfg(test)]
mod compile_tests;
#[cfg(test)]
mod graph_tests;
