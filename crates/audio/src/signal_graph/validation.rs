use std::collections::BTreeSet;

use crate::AudioError;

use super::compile::{MAX_EDGES, MAX_NODES};
use super::graph::{Edge, Node, NodeId, NodeKind, NodeProvenance, SignalGraph, TrackStage};

pub(super) fn validate(graph: &SignalGraph) -> Result<(), AudioError> {
    validate_limits(graph)?;
    validate_nodes(graph)?;
    validate_edges(graph)?;
    validate_fixed_topology(graph)?;
    validate_cardinality(graph)?;
    validate_acyclic(graph)
}

fn invalid(issue: &'static str) -> AudioError {
    AudioError::InvalidSignalGraph { issue }
}

fn validate_limits(graph: &SignalGraph) -> Result<(), AudioError> {
    for (resource, actual, limit) in [
        ("nodes", graph.nodes.len(), MAX_NODES),
        ("edges", graph.edges.len(), MAX_EDGES),
    ] {
        if actual > limit {
            return Err(AudioError::SignalGraphLimitExceeded {
                resource,
                limit,
                actual,
            });
        }
    }
    Ok(())
}

fn validate_nodes(graph: &SignalGraph) -> Result<(), AudioError> {
    if graph.nodes.get(graph.master.0).map(|node| node.kind) != Some(NodeKind::Master) {
        return Err(invalid("master node is missing"));
    }
    let mut master_count = 0;
    for (index, node) in graph.nodes.iter().enumerate() {
        if node.id != NodeId(index) {
            return Err(invalid("node IDs are not dense"));
        }
        if node.kind == NodeKind::Master {
            master_count += 1;
        }
        if node.latency_frames != node.kind.intrinsic_latency_frames() || node.latency_frames != 0 {
            return Err(invalid("node has unsupported latency"));
        }
        validate_provenance(node)?;
    }
    if master_count != 1 {
        return Err(invalid("graph must contain exactly one master"));
    }
    validate_track_nodes(graph)
}

fn validate_track_nodes(graph: &SignalGraph) -> Result<(), AudioError> {
    if graph.nodes.len() % 3 != 1 {
        return Err(invalid("track nodes are not complete"));
    }
    for (ordinal, track_nodes) in graph.nodes[1..].chunks_exact(3).enumerate() {
        let track_id = match &track_nodes[0].provenance {
            NodeProvenance::Track { track_id, .. } => track_id,
            NodeProvenance::Master => return Err(invalid("track node has master provenance")),
        };
        for (node, (kind, stage)) in track_nodes.iter().zip([
            (NodeKind::TrackSource, TrackStage::Source),
            (NodeKind::OnePoleLowPass, TrackStage::LowPass),
            (NodeKind::GainPan, TrackStage::GainPan),
        ]) {
            if node.kind != kind
                || !matches!(
                    &node.provenance,
                    NodeProvenance::Track {
                        ordinal: node_ordinal,
                        track_id: node_track_id,
                        stage: node_stage,
                    } if *node_ordinal == ordinal && node_track_id == track_id && *node_stage == stage
                )
            {
                return Err(invalid("track nodes are not in fixed stage order"));
            }
        }
    }
    Ok(())
}

fn validate_provenance(node: &Node) -> Result<(), AudioError> {
    let valid = matches!(
        (&node.kind, &node.provenance),
        (NodeKind::Master, NodeProvenance::Master)
            | (
                NodeKind::TrackSource,
                NodeProvenance::Track {
                    stage: TrackStage::Source,
                    ..
                }
            )
            | (
                NodeKind::OnePoleLowPass,
                NodeProvenance::Track {
                    stage: TrackStage::LowPass,
                    ..
                }
            )
            | (
                NodeKind::GainPan,
                NodeProvenance::Track {
                    stage: TrackStage::GainPan,
                    ..
                }
            )
    );
    if valid {
        Ok(())
    } else {
        Err(invalid("node provenance does not match node kind"))
    }
}

fn validate_edges(graph: &SignalGraph) -> Result<(), AudioError> {
    let mut unique = BTreeSet::new();
    for edge in &graph.edges {
        let Some(source) = graph.nodes.get(edge.from.node.0) else {
            return Err(invalid("edge has dangling source"));
        };
        let Some(target) = graph.nodes.get(edge.to.node.0) else {
            return Err(invalid("edge has dangling destination"));
        };
        if !unique.insert(*edge) {
            return Err(invalid("graph contains a duplicate edge"));
        }
        validate_edge_shape(source, target, edge.to.port)?;
    }
    Ok(())
}

fn validate_edge_shape(source: &Node, target: &Node, port: usize) -> Result<(), AudioError> {
    let valid = match (source.kind, target.kind) {
        (NodeKind::TrackSource, NodeKind::OnePoleLowPass)
        | (NodeKind::OnePoleLowPass, NodeKind::GainPan) => port == 0,
        (NodeKind::GainPan, NodeKind::Master) => match &source.provenance {
            NodeProvenance::Track { ordinal, .. } => port == *ordinal,
            NodeProvenance::Master => false,
        },
        (NodeKind::TrackSource, NodeKind::TrackSource)
        | (NodeKind::TrackSource, NodeKind::GainPan)
        | (NodeKind::TrackSource, NodeKind::Master)
        | (NodeKind::OnePoleLowPass, NodeKind::TrackSource)
        | (NodeKind::OnePoleLowPass, NodeKind::OnePoleLowPass)
        | (NodeKind::OnePoleLowPass, NodeKind::Master)
        | (NodeKind::GainPan, NodeKind::TrackSource)
        | (NodeKind::GainPan, NodeKind::OnePoleLowPass)
        | (NodeKind::GainPan, NodeKind::GainPan)
        | (NodeKind::Master, NodeKind::TrackSource)
        | (NodeKind::Master, NodeKind::OnePoleLowPass)
        | (NodeKind::Master, NodeKind::GainPan)
        | (NodeKind::Master, NodeKind::Master) => false,
    };
    if valid {
        Ok(())
    } else {
        Err(invalid("edge violates fixed topology"))
    }
}

fn validate_fixed_topology(graph: &SignalGraph) -> Result<(), AudioError> {
    let expected_tracks = (graph.nodes.len() - 1) / 3;
    if graph.edges.len() != expected_tracks * 3 {
        return Err(invalid("fixed topology edge count is invalid"));
    }
    for ordinal in 0..expected_tracks {
        let source = NodeId(1 + ordinal * 3);
        let expected = [
            Edge::new(source, NodeId(source.0 + 1), 0),
            Edge::new(NodeId(source.0 + 1), NodeId(source.0 + 2), 0),
            Edge::new(NodeId(source.0 + 2), graph.master, ordinal),
        ];
        if expected.iter().any(|edge| !graph.edges.contains(edge)) {
            return Err(invalid("fixed track path is incomplete"));
        }
    }
    Ok(())
}

fn validate_cardinality(graph: &SignalGraph) -> Result<(), AudioError> {
    for node in &graph.nodes {
        let incoming = graph
            .edges
            .iter()
            .filter(|edge| edge.to.node == node.id)
            .count();
        let outgoing = graph
            .edges
            .iter()
            .filter(|edge| edge.from.node == node.id)
            .count();
        let valid = match node.kind {
            NodeKind::TrackSource => incoming == 0 && outgoing == 1,
            NodeKind::OnePoleLowPass | NodeKind::GainPan => incoming == 1 && outgoing == 1,
            NodeKind::Master => incoming * 3 + 1 == graph.nodes.len() && outgoing == 0,
        };
        if !valid {
            return Err(invalid("node has invalid edge cardinality"));
        }
    }
    Ok(())
}

fn validate_acyclic(graph: &SignalGraph) -> Result<(), AudioError> {
    let mut indegree = vec![0_usize; graph.nodes.len()];
    for edge in &graph.edges {
        indegree[edge.to.node.0] += 1;
    }
    let mut stack: Vec<_> = indegree
        .iter()
        .enumerate()
        .filter_map(|(index, degree)| (*degree == 0).then_some(index))
        .collect();
    let mut visited = 0;
    while let Some(node) = stack.pop() {
        visited += 1;
        for edge in graph.edges.iter().filter(|edge| edge.from.node.0 == node) {
            indegree[edge.to.node.0] -= 1;
            if indegree[edge.to.node.0] == 0 {
                stack.push(edge.to.node.0);
            }
        }
    }
    if visited == graph.nodes.len() {
        Ok(())
    } else {
        Err(invalid("graph contains a cycle"))
    }
}
