use crate::ids::{DefinitionId, InstanceId, Revision, TrackId};
use crate::music::Instance;
use crate::project::Project;
use crate::time::Tick;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeImpact {
    pub base_revision: Revision,
    pub affected_definitions: Vec<DefinitionId>,
    pub affected_tracks: Vec<TrackId>,
    pub preview_ranges: Vec<InstancePreviewRange>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstancePreviewRange {
    instance_id: InstanceId,
    before: Option<BeatRange>,
    after: Option<BeatRange>,
}

#[derive(Serialize)]
pub struct BeatRange {
    start: Tick,
    end: Tick,
}

impl From<&Instance> for BeatRange {
    fn from(instance: &Instance) -> Self {
        Self {
            start: instance.start,
            end: instance.start + instance.length,
        }
    }
}

pub fn analyze(
    before: &Project,
    after: &Project,
    requested: &[InstanceId],
) -> (Vec<InstanceId>, ChangeImpact) {
    let old: BTreeMap<_, _> = before
        .instances
        .iter()
        .map(|instance| (&instance.id, instance))
        .collect();
    let new: BTreeMap<_, _> = after
        .instances
        .iter()
        .map(|instance| (&instance.id, instance))
        .collect();
    let old_solo: BTreeSet<_> = before
        .tracks
        .iter()
        .filter(|track| track.solo)
        .map(|track| &track.id)
        .collect();
    let new_solo: BTreeSet<_> = after
        .tracks
        .iter()
        .filter(|track| track.solo)
        .map(|track| &track.id)
        .collect();
    let mut affected: BTreeSet<_> = requested.iter().cloned().collect();
    let definition_ids: BTreeSet<_> = before
        .definitions
        .iter()
        .map(|definition| definition.id.clone())
        .chain(
            after
                .definitions
                .iter()
                .map(|definition| definition.id.clone()),
        )
        .collect();
    let changed_definitions = definition_ids.into_iter().filter(|id| {
        before.definitions.iter().find(|item| item.id == *id)
            != after.definitions.iter().find(|item| item.id == *id)
    });
    let changed_definitions: Vec<_> = changed_definitions.collect();
    let mut definition_closure =
        crate::definition_graph::ancestor_closure(before, changed_definitions.iter().cloned());
    definition_closure.extend(crate::definition_graph::ancestor_closure(
        after,
        changed_definitions,
    ));
    for (id, instance) in old.iter().chain(&new) {
        let definition = &instance.definition_id;
        let track = &instance.track_id;
        if old.get(id) != new.get(id)
            || before.tempo != after.tempo
            || before.tempo_map != after.tempo_map
            || before.meter != after.meter
            || before.meter_map != after.meter_map
            || old_solo != new_solo
            || definition_closure.contains(definition)
            || before.tracks.iter().find(|item| item.id == *track)
                != after.tracks.iter().find(|item| item.id == *track)
        {
            affected.insert((*id).clone());
        }
    }
    let mut definitions = BTreeSet::new();
    let mut tracks = BTreeSet::new();
    let mut ranges = Vec::new();
    for id in &affected {
        if let Some(instance) = old.get(id) {
            definitions.extend(crate::definition_graph::descendant_closure(
                before,
                [instance.definition_id.clone()],
            ));
            tracks.insert(instance.track_id.clone());
        }
        if let Some(instance) = new.get(id) {
            definitions.extend(crate::definition_graph::descendant_closure(
                after,
                [instance.definition_id.clone()],
            ));
            tracks.insert(instance.track_id.clone());
        }
        ranges.push(InstancePreviewRange {
            instance_id: id.clone(),
            before: old.get(id).map(|instance| BeatRange::from(*instance)),
            after: new.get(id).map(|instance| BeatRange::from(*instance)),
        });
    }
    definitions.extend(definition_closure);
    (
        affected.into_iter().collect(),
        ChangeImpact {
            base_revision: before.revision.clone(),
            affected_definitions: definitions.into_iter().collect(),
            affected_tracks: tracks.into_iter().collect(),
            preview_ranges: ranges,
        },
    )
}
