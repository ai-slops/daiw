use crate::AudioError;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct NodeId(pub(crate) usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NodeKind {
    TrackSource,
    OnePoleLowPass,
    GainPan,
    Master,
}

impl NodeKind {
    pub(crate) const fn intrinsic_latency_frames(self) -> usize {
        match self {
            Self::TrackSource | Self::OnePoleLowPass | Self::GainPan | Self::Master => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TrackStage {
    Source,
    LowPass,
    GainPan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum NodeProvenance {
    Master,
    Track {
        ordinal: usize,
        track_id: String,
        stage: TrackStage,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Node {
    pub(crate) id: NodeId,
    pub(crate) kind: NodeKind,
    pub(crate) latency_frames: usize,
    pub(crate) provenance: NodeProvenance,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct StereoOutput {
    pub(crate) node: NodeId,
}

impl StereoOutput {
    pub(crate) const fn new(node: NodeId) -> Self {
        Self { node }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct StereoInput {
    pub(crate) node: NodeId,
    pub(crate) port: usize,
}

impl StereoInput {
    pub(crate) const fn new(node: NodeId, port: usize) -> Self {
        Self { node, port }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct Edge {
    pub(crate) from: StereoOutput,
    pub(crate) to: StereoInput,
}

impl Edge {
    pub(crate) const fn new(from: NodeId, to: NodeId, input_port: usize) -> Self {
        Self {
            from: StereoOutput::new(from),
            to: StereoInput::new(to, input_port),
        }
    }
}

#[derive(Debug)]
pub(crate) struct SignalGraph {
    pub(crate) nodes: Vec<Node>,
    pub(crate) edges: Vec<Edge>,
    pub(crate) master: NodeId,
}

impl SignalGraph {
    pub(crate) fn validate(&self) -> Result<(), AudioError> {
        super::validation::validate(self)
    }
}
