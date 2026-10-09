use crate::error::CoreError;
use crate::ids::DefinitionId;
use crate::project::Project;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_DEFINITION_DEPTH: usize = 16;
pub const MAX_CHILD_PLACEMENTS: usize = 4_096;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionRevision {
    pub definition_id: DefinitionId,
    pub revision: u64,
}

pub fn definition_revisions(
    project: &Project,
    root: &DefinitionId,
) -> Result<Vec<DefinitionRevision>, CoreError> {
    let definitions: BTreeMap<_, _> = project
        .definitions
        .iter()
        .map(|definition| (&definition.id, definition))
        .collect();
    let mut reachable = BTreeSet::new();
    collect(root, &definitions, &mut reachable)?;
    Ok(reachable
        .into_iter()
        .map(|id| DefinitionRevision {
            definition_id: id.clone(),
            revision: definitions[&id].revision,
        })
        .collect())
}

pub fn nested_revisions(
    project: &Project,
    root: &DefinitionId,
) -> Result<Option<Vec<DefinitionRevision>>, CoreError> {
    let revisions = definition_revisions(project, root)?;
    Ok((revisions.len() > 1).then_some(revisions))
}

pub fn revisions_are_sorted(revisions: &[DefinitionRevision]) -> bool {
    !revisions.is_empty()
        && revisions
            .windows(2)
            .all(|pair| pair[0].definition_id < pair[1].definition_id)
}

pub fn revision_closure_matches(
    captured: Option<&[DefinitionRevision]>,
    current: &[DefinitionRevision],
) -> bool {
    captured.map_or(current.len() == 1, |saved| saved == current)
}

pub fn ancestor_closure(
    project: &Project,
    seeds: impl IntoIterator<Item = DefinitionId>,
) -> BTreeSet<DefinitionId> {
    let mut closure: BTreeSet<_> = seeds.into_iter().collect();
    loop {
        let before = closure.len();
        for definition in &project.definitions {
            if definition
                .children
                .iter()
                .any(|child| closure.contains(&child.definition_id))
            {
                closure.insert(definition.id.clone());
            }
        }
        if closure.len() == before {
            return closure;
        }
    }
}

pub fn descendant_closure(
    project: &Project,
    seeds: impl IntoIterator<Item = DefinitionId>,
) -> BTreeSet<DefinitionId> {
    let mut closure: BTreeSet<_> = seeds.into_iter().collect();
    loop {
        let before = closure.len();
        for definition in &project.definitions {
            if closure.contains(&definition.id) {
                closure.extend(
                    definition
                        .children
                        .iter()
                        .map(|child| child.definition_id.clone()),
                );
            }
        }
        if closure.len() == before {
            return closure;
        }
    }
}

pub fn dependent_instances(
    project: &Project,
    definition: &DefinitionId,
) -> Vec<crate::ids::InstanceId> {
    let affected = ancestor_closure(project, [definition.clone()]);
    project
        .instances
        .iter()
        .filter(|instance| affected.contains(&instance.definition_id))
        .map(|instance| instance.id.clone())
        .collect()
}

pub fn validate(project: &Project) -> Result<(), CoreError> {
    let total = project
        .definitions
        .iter()
        .try_fold(0_usize, |count, definition| {
            count.checked_add(definition.children.len())
        })
        .ok_or_else(|| CoreError::InvalidProject("child placement count overflow".into()))?;
    if total > MAX_CHILD_PLACEMENTS {
        return Err(CoreError::InvalidProject(format!(
            "definition child placement limit exceeded: {total} > {MAX_CHILD_PLACEMENTS}"
        )));
    }
    let definitions: BTreeMap<_, _> = project
        .definitions
        .iter()
        .map(|definition| (&definition.id, definition))
        .collect();
    let mut placement_ids = BTreeSet::new();
    for definition in &project.definitions {
        for child in &definition.children {
            if !placement_ids.insert(&child.id) {
                return Err(CoreError::InvalidProject(format!(
                    "duplicate child placement id {}",
                    child.id
                )));
            }
            if !definitions.contains_key(&child.definition_id) {
                return Err(CoreError::InvalidProject(format!(
                    "child placement {} references missing definition",
                    child.id
                )));
            }
            let end = child.start.ticks().checked_add(child.length.ticks());
            if child.start.ticks() < 0
                || child.length.ticks() <= 0
                || end.is_none_or(|value| value > definition.length.ticks())
            {
                return Err(CoreError::InvalidProject(format!(
                    "child placement {} is out of range",
                    child.id
                )));
            }
        }
    }
    let mut walker = GraphWalker {
        definitions: &definitions,
        visiting: BTreeSet::new(),
        longest_depth: BTreeMap::new(),
    };
    for definition in &project.definitions {
        walker.walk(&definition.id, 0)?;
    }
    Ok(())
}

struct GraphWalker<'a> {
    definitions: &'a BTreeMap<&'a DefinitionId, &'a crate::music::Definition>,
    visiting: BTreeSet<&'a DefinitionId>,
    longest_depth: BTreeMap<&'a DefinitionId, usize>,
}

impl<'a> GraphWalker<'a> {
    fn walk(&mut self, id: &'a DefinitionId, entrance_depth: usize) -> Result<usize, CoreError> {
        if entrance_depth > MAX_DEFINITION_DEPTH {
            return Err(depth_error());
        }
        if let Some(longest) = self.longest_depth.get(id) {
            if entrance_depth
                .checked_add(*longest)
                .is_none_or(|depth| depth > MAX_DEFINITION_DEPTH)
            {
                return Err(depth_error());
            }
            return Ok(*longest);
        }
        if !self.visiting.insert(id) {
            return Err(CoreError::InvalidProject(format!(
                "definition composition cycle at {id}"
            )));
        }
        let definition = self
            .definitions
            .get(id)
            .ok_or_else(|| CoreError::NotFound(id.to_string()))?;
        let mut longest = 0;
        for child in &definition.children {
            let child_depth = self.walk(&child.definition_id, entrance_depth + 1)?;
            longest = longest.max(child_depth + 1);
        }
        self.visiting.remove(id);
        self.longest_depth.insert(id, longest);
        Ok(longest)
    }
}

fn depth_error() -> CoreError {
    CoreError::InvalidProject(format!(
        "definition composition depth exceeds {MAX_DEFINITION_DEPTH}"
    ))
}

fn collect<'a>(
    id: &'a DefinitionId,
    definitions: &BTreeMap<&'a DefinitionId, &'a crate::music::Definition>,
    reachable: &mut BTreeSet<&'a DefinitionId>,
) -> Result<(), CoreError> {
    if !reachable.insert(id) {
        return Ok(());
    }
    let definition = definitions
        .get(id)
        .ok_or_else(|| CoreError::NotFound(id.to_string()))?;
    for child in &definition.children {
        collect(&child.definition_id, definitions, reachable)?;
    }
    Ok(())
}
