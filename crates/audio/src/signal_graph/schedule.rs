use crate::AudioError;

use super::compile::{Instruction, MAX_INSTRUCTIONS};
use super::graph::{Edge, Node, NodeId, NodeKind, NodeProvenance, SignalGraph};

struct TrackPath<'a> {
    source: &'a Node,
    filter: &'a Node,
    gain: &'a Node,
}

pub(super) fn compile_schedule(graph: &SignalGraph) -> Result<Vec<Instruction>, AudioError> {
    let track_count = graph
        .edges
        .iter()
        .filter(|edge| edge.to.node == graph.master)
        .count();
    let instruction_count = track_count
        .checked_mul(5)
        .and_then(|count| count.checked_add(2))
        .ok_or(AudioError::SignalGraphLimitExceeded {
            resource: "instructions",
            limit: MAX_INSTRUCTIONS,
            actual: usize::MAX,
        })?;
    if instruction_count > MAX_INSTRUCTIONS {
        return Err(AudioError::SignalGraphLimitExceeded {
            resource: "instructions",
            limit: MAX_INSTRUCTIONS,
            actual: instruction_count,
        });
    }
    let mut master_inputs: Vec<&Edge> = Vec::new();
    master_inputs.try_reserve_exact(track_count)?;
    master_inputs.extend(
        graph
            .edges
            .iter()
            .filter(|edge| edge.to.node == graph.master),
    );
    master_inputs.sort_unstable_by_key(|edge| edge.to.port);
    let mut instructions = Vec::new();
    instructions.try_reserve_exact(instruction_count)?;
    instructions.push(Instruction::ClearMaster);
    for edge in master_inputs {
        let gain = node(graph, edge.from.node)?;
        let filter = node(graph, input_source(graph, gain.id)?)?;
        let source = node(graph, input_source(graph, filter.id)?)?;
        let track = track_ordinal(gain)?;
        validate_path(
            TrackPath {
                source,
                filter,
                gain,
            },
            track,
        )?;
        instructions.extend([
            Instruction::ClearTrack { track },
            Instruction::RenderTrackSource { track },
            Instruction::LowPass { track },
            Instruction::ValidateTrack { track },
            Instruction::GainPanIntoMaster { track },
        ]);
    }
    instructions.push(Instruction::ValidateMaster);
    Ok(instructions)
}

fn node(graph: &SignalGraph, id: NodeId) -> Result<&Node, AudioError> {
    graph.nodes.get(id.0).ok_or(AudioError::InvalidSignalGraph {
        issue: "schedule references a missing node",
    })
}

fn input_source(graph: &SignalGraph, target: NodeId) -> Result<NodeId, AudioError> {
    let mut incoming = graph.edges.iter().filter(|edge| edge.to.node == target);
    let source =
        incoming
            .next()
            .map(|edge| edge.from.node)
            .ok_or(AudioError::InvalidSignalGraph {
                issue: "schedule path has a missing input",
            })?;
    if incoming.next().is_some() {
        return Err(AudioError::InvalidSignalGraph {
            issue: "schedule path has multiple inputs",
        });
    }
    Ok(source)
}

fn track_ordinal(node: &Node) -> Result<usize, AudioError> {
    match &node.provenance {
        NodeProvenance::Track { ordinal, .. } => Ok(*ordinal),
        NodeProvenance::Master => Err(AudioError::InvalidSignalGraph {
            issue: "track schedule contains master provenance",
        }),
    }
}

fn validate_path(path: TrackPath<'_>, track: usize) -> Result<(), AudioError> {
    let kinds_match = path.source.kind == NodeKind::TrackSource
        && path.filter.kind == NodeKind::OnePoleLowPass
        && path.gain.kind == NodeKind::GainPan;
    let ordinals_match = [path.source, path.filter, path.gain]
        .into_iter()
        .all(|node| matches!(track_ordinal(node), Ok(ordinal) if ordinal == track));
    if kinds_match && ordinals_match {
        Ok(())
    } else {
        Err(AudioError::InvalidSignalGraph {
            issue: "schedule path does not match fixed topology",
        })
    }
}
