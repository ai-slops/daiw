use crate::music::Operation;
use crate::project::Project;
use std::collections::BTreeSet;

pub const SUPPORTED: &[&str] = &[
    "audio-regions-v1",
    "core-v1",
    "meter-map-v1",
    "nested-definitions-v1",
    "modifier-ordering-v1",
    "named-slots-v1",
    "note-patches-v1",
    "proposal-review-v1",
    "slot-bindings-v1",
    "tempo-map-v1",
];

pub fn required(project: &Project) -> BTreeSet<&'static str> {
    let mut features = BTreeSet::from(["core-v1"]);
    if project.tempo_map.is_some() {
        features.insert("tempo-map-v1");
    }
    if project.meter_map.is_some() {
        features.insert("meter-map-v1");
    }
    if !project.assets.is_empty() || !project.regions.is_empty() {
        features.insert("audio-regions-v1");
    }
    if !project.proposals.is_empty() {
        features.insert("proposal-review-v1");
    }
    if project
        .definitions
        .iter()
        .any(|definition| !definition.slots.is_empty())
    {
        features.insert("named-slots-v1");
    }
    if project
        .definitions
        .iter()
        .any(|definition| !definition.children.is_empty())
    {
        features.insert("nested-definitions-v1");
    }
    for instance in &project.instances {
        if instance.modifier_ordering.is_some() {
            features.insert("modifier-ordering-v1");
        }
        if !instance.modifier_bindings.is_empty() {
            features.insert("slot-bindings-v1");
        }
        if instance
            .modifier_bindings
            .values()
            .any(|binding| binding.definition_revisions.is_some())
        {
            features.insert("nested-definitions-v1");
        }
        if instance
            .modifier_bindings
            .values()
            .any(|binding| binding.slots.keys().any(|slot| slot != "notes"))
        {
            features.insert("named-slots-v1");
        }
    }
    for hunk in project
        .proposals
        .iter()
        .flat_map(|proposal| &proposal.hunks)
    {
        if let Some(binding) = &hunk.slot_binding {
            features.insert("slot-bindings-v1");
            if binding.slot != "notes" {
                features.insert("named-slots-v1");
            }
            if binding.definition_revisions.is_some() {
                features.insert("nested-definitions-v1");
            }
        }
    }
    if project.proposals.iter().any(|proposal| {
        proposal
            .base_anchor
            .as_ref()
            .is_some_and(|anchor| anchor.definition_revisions.is_some())
    }) {
        features.insert("nested-definitions-v1");
    }
    let operations = project
        .modifiers
        .iter()
        .flat_map(|modifier| &modifier.operations)
        .chain(
            project
                .instances
                .iter()
                .flat_map(|instance| &instance.overrides),
        )
        .chain(
            project
                .proposals
                .iter()
                .flat_map(|proposal| proposal.hunks.iter().map(|hunk| &hunk.operation)),
        );
    for operation in operations {
        match operation {
            Operation::PatchNotes(_) => {
                features.insert("note-patches-v1");
            }
            Operation::ReplaceSlot { slot, .. } => {
                if slot != "notes" {
                    features.insert("named-slots-v1");
                }
            }
            Operation::Transpose { .. }
            | Operation::Velocity { .. }
            | Operation::Gain { .. }
            | Operation::AddNotes { .. }
            | Operation::SetParameter { .. } => {}
        }
    }
    features
}
