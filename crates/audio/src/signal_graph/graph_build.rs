use super::graph::{Edge, Node, NodeId, NodeKind, NodeProvenance, TrackStage};

pub(super) fn append_track_graph(
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
