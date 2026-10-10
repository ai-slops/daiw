use crate::AudioError;

use super::graph::SignalGraph;

pub(super) fn compile_latency(graph: &SignalGraph) -> Result<usize, AudioError> {
    let mut latency_frames = 0_usize;
    for node in &graph.nodes {
        latency_frames = latency_frames.checked_add(node.latency_frames).ok_or(
            AudioError::InvalidSignalGraph {
                issue: "signal graph latency overflows frame count",
            },
        )?;
    }
    if latency_frames == 0 {
        Ok(0)
    } else {
        Err(AudioError::InvalidSignalGraph {
            issue: "nonzero signal graph latency is unsupported",
        })
    }
}
