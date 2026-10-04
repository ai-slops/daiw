use crate::error::CoreError;
use crate::ids::ModifierId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ModifierOrdering {
    pub edges: Vec<ModifierOrderEdge>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ModifierOrderEdge {
    pub before: ModifierId,
    pub after: ModifierId,
}

pub struct OrderedGraph {
    pub order: Vec<ModifierId>,
    reachable: BTreeMap<ModifierId, BTreeSet<ModifierId>>,
}

impl OrderedGraph {
    pub fn relates(&self, a: &ModifierId, b: &ModifierId) -> bool {
        self.reachable.get(a).is_some_and(|ids| ids.contains(b))
            || self.reachable.get(b).is_some_and(|ids| ids.contains(a))
    }
}

pub fn resolve(ids: &[ModifierId], graph: &ModifierOrdering) -> Result<OrderedGraph, CoreError> {
    if ids.len() > 256 || graph.edges.len() > 4096 {
        return Err(CoreError::ModifierOrderingInvalid(
            "graph exceeds 256 nodes or 4096 edges".into(),
        ));
    }
    let nodes: BTreeSet<_> = ids.iter().cloned().collect();
    if nodes.len() != ids.len() {
        return Err(CoreError::ModifierOrderingInvalid(
            "duplicate attached modifier ID".into(),
        ));
    }
    let mut pending: BTreeMap<_, BTreeSet<_>> = nodes
        .iter()
        .map(|id| (id.clone(), BTreeSet::new()))
        .collect();
    for edge in &graph.edges {
        if edge.before == edge.after || !nodes.contains(&edge.before) {
            return Err(CoreError::ModifierOrderingInvalid(
                "self-edge or unattached endpoint".into(),
            ));
        }
        let predecessors = pending
            .get_mut(&edge.after)
            .ok_or_else(|| CoreError::ModifierOrderingInvalid("unattached endpoint".into()))?;
        if !predecessors.insert(edge.before.clone()) {
            return Err(CoreError::ModifierOrderingInvalid(
                "duplicate dependency edge".into(),
            ));
        }
    }
    let mut order = Vec::with_capacity(nodes.len());
    while !pending.is_empty() {
        let id = pending
            .iter()
            .find(|(_, parents)| parents.is_empty())
            .map(|(id, _)| id.clone())
            .ok_or(CoreError::ModifierOrderingCycle)?;
        pending.remove(&id);
        for predecessors in pending.values_mut() {
            predecessors.remove(&id);
        }
        order.push(id);
    }
    let mut reachable: BTreeMap<ModifierId, BTreeSet<ModifierId>> = BTreeMap::new();
    for id in order.iter().rev() {
        let mut descendants = BTreeSet::new();
        for edge in graph.edges.iter().filter(|edge| edge.before == *id) {
            descendants.insert(edge.after.clone());
            if let Some(inherited) = reachable.get(&edge.after) {
                descendants.extend(inherited.iter().cloned());
            }
        }
        reachable.insert(id.clone(), descendants);
    }
    Ok(OrderedGraph { order, reachable })
}
