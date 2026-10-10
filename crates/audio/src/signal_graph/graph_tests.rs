use super::{Edge, Node, NodeId, NodeKind, NodeProvenance, SignalGraph, StereoInput, TrackStage};
use crate::AudioError;

fn track_provenance(stage: TrackStage) -> NodeProvenance {
    NodeProvenance::Track {
        ordinal: 0,
        track_id: "track-0".into(),
        stage,
    }
}

fn valid_graph() -> SignalGraph {
    SignalGraph {
        nodes: vec![
            Node {
                id: NodeId(0),
                kind: NodeKind::Master,
                latency_frames: 0,
                provenance: NodeProvenance::Master,
            },
            Node {
                id: NodeId(1),
                kind: NodeKind::TrackSource,
                latency_frames: 0,
                provenance: track_provenance(TrackStage::Source),
            },
            Node {
                id: NodeId(2),
                kind: NodeKind::OnePoleLowPass,
                latency_frames: 0,
                provenance: track_provenance(TrackStage::LowPass),
            },
            Node {
                id: NodeId(3),
                kind: NodeKind::GainPan,
                latency_frames: 0,
                provenance: track_provenance(TrackStage::GainPan),
            },
        ],
        edges: vec![
            Edge::new(NodeId(1), NodeId(2), 0),
            Edge::new(NodeId(2), NodeId(3), 0),
            Edge::new(NodeId(3), NodeId(0), 0),
        ],
        master: NodeId(0),
    }
}

fn assert_invalid(graph: &SignalGraph) {
    assert!(matches!(
        graph.validate(),
        Err(AudioError::InvalidSignalGraph { .. })
    ));
}

#[test]
fn graph_rejects_dangling_endpoint() {
    // Given
    let mut graph = valid_graph();
    graph.edges[0].to = StereoInput::new(NodeId(99), 0);

    // When / Then
    assert_invalid(&graph);
}

#[test]
fn graph_rejects_duplicate_edge() {
    // Given
    let mut graph = valid_graph();
    graph.edges.push(graph.edges[0]);

    // When / Then
    assert_invalid(&graph);
}

#[test]
fn graph_rejects_wrong_cardinality() {
    // Given
    let mut graph = valid_graph();
    graph.edges.remove(1);

    // When / Then
    assert_invalid(&graph);
}

#[test]
fn graph_rejects_master_input_out_of_track_order() {
    // Given
    let mut graph = valid_graph();
    graph.edges[2].to = StereoInput::new(NodeId(0), 1);

    // When / Then
    assert_invalid(&graph);
}

#[test]
fn graph_rejects_non_dense_node_ids() {
    // Given
    let mut graph = valid_graph();
    graph.nodes[2].id = NodeId(7);

    // When / Then
    assert_invalid(&graph);
}

#[test]
fn graph_rejects_nonzero_intrinsic_latency() {
    // Given
    let mut graph = valid_graph();
    graph.nodes[1].latency_frames = 1;

    // When / Then
    assert_invalid(&graph);
}
