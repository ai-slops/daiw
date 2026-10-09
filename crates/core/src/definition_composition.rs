use crate::error::CoreError;
use crate::ids::{DefinitionId, NoteId};
use crate::music::{Definition, DefinitionPathEntry, Note};
use crate::project::Project;
use crate::time::Tick;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub const COMPOSITION_WORK_LIMIT: u64 = 100_000;
const NOTE_ID_DOMAIN: &[u8] = b"daiw:nested-pattern-note-id:v1\0";

#[derive(Clone)]
pub struct ComposedNote {
    pub note: Note,
    pub definition_id: DefinitionId,
    pub source_note_id: NoteId,
    pub definition_path: Option<Vec<DefinitionPathEntry>>,
}

pub struct WorkMeter {
    used: u64,
    limit: u64,
}

impl WorkMeter {
    pub const fn new() -> Self {
        Self {
            used: 0,
            limit: COMPOSITION_WORK_LIMIT,
        }
    }

    #[cfg(test)]
    pub const fn with_limit(limit: u64) -> Self {
        Self { used: 0, limit }
    }

    #[cfg(test)]
    pub const fn with_usage(used: u64, limit: u64) -> Self {
        Self { used, limit }
    }

    pub fn consume(&mut self, amount: usize) -> Result<(), CoreError> {
        let amount = u64::try_from(amount)
            .map_err(|_| CoreError::InvalidProject("composition work exceeds platform".into()))?;
        self.used = self
            .used
            .checked_add(amount)
            .ok_or_else(arithmetic_overflow)?;
        if self.used > self.limit {
            return Err(CoreError::InvalidProject(format!(
                "definition composition work budget exceeded: {} > {}",
                self.used, self.limit
            )));
        }
        Ok(())
    }

    pub(crate) const fn used(&self) -> u64 {
        self.used
    }

    pub(crate) fn consume_replayed(&mut self, amount: u64) -> Result<(), CoreError> {
        if amount == 0 {
            return Ok(());
        }
        if self.used > self.limit {
            self.used = self.used.checked_add(1).ok_or_else(arithmetic_overflow)?;
            return Err(CoreError::InvalidProject(format!(
                "definition composition work budget exceeded: {} > {}",
                self.used, self.limit
            )));
        }
        let remaining = self.limit - self.used;
        if amount <= remaining {
            self.used = self
                .used
                .checked_add(amount)
                .ok_or_else(arithmetic_overflow)?;
            return Ok(());
        }
        let Some(exceeded) = self.limit.checked_add(1) else {
            self.used = self.limit;
            return Err(arithmetic_overflow());
        };
        self.used = exceeded;
        Err(CoreError::InvalidProject(format!(
            "definition composition work budget exceeded: {} > {}",
            self.used, self.limit
        )))
    }
}

struct Composer<'a, 'work> {
    root: &'a DefinitionId,
    definitions: BTreeMap<&'a DefinitionId, &'a Definition>,
    work: &'work mut WorkMeter,
    path: Vec<DefinitionPathEntry>,
}

#[derive(Serialize)]
struct PatternIdentity<'a> {
    root_definition_id: &'a str,
    path: Vec<(&'a str, u64)>,
    leaf_definition_id: &'a str,
    leaf_note_id: &'a str,
}

pub fn compose_metered(
    project: &Project,
    root: &Definition,
    work: &mut WorkMeter,
) -> Result<Vec<ComposedNote>, CoreError> {
    let mut composer = Composer {
        root: &root.id,
        definitions: project
            .definitions
            .iter()
            .map(|definition| (&definition.id, definition))
            .collect(),
        work,
        path: vec![DefinitionPathEntry {
            definition_id: root.id.clone(),
            definition_revision: root.revision,
            placement_id: None,
            loop_index: None,
        }],
    };
    let mut output = Vec::new();
    composer.expand(root, &mut output)?;
    let mut ids = BTreeSet::new();
    if output.iter().any(|item| !ids.insert(&item.note.id)) {
        return Err(CoreError::InvalidProject(format!(
            "composed note id collision in definition {}",
            root.id
        )));
    }
    if !root.children.is_empty() {
        output.sort_by(|left, right| {
            left.note
                .start
                .cmp(&right.note.start)
                .then_with(|| left.note.id.cmp(&right.note.id))
        });
    }
    Ok(output)
}

impl Composer<'_, '_> {
    fn expand(
        &mut self,
        definition: &Definition,
        output: &mut Vec<ComposedNote>,
    ) -> Result<(), CoreError> {
        let is_root = definition.id == *self.root && self.path.len() == 1;
        for source in &definition.notes {
            self.consume_work()?;
            let mut note = source.clone();
            if !is_root {
                note.id = self.nested_note_id(&definition.id, &source.id)?;
            }
            output.push(ComposedNote {
                note,
                definition_id: definition.id.clone(),
                source_note_id: source.id.clone(),
                definition_path: (!is_root).then(|| self.path.clone()),
            });
        }
        for child in &definition.children {
            let child_definition = self
                .definitions
                .get(&child.definition_id)
                .copied()
                .ok_or_else(|| CoreError::NotFound(child.definition_id.to_string()))?;
            let mut loop_start = 0_i64;
            let mut loop_index = 0_u64;
            while loop_start < child.length.ticks() {
                self.consume_work()?;
                self.path.push(DefinitionPathEntry {
                    definition_id: child_definition.id.clone(),
                    definition_revision: child_definition.revision,
                    placement_id: Some(child.id.clone()),
                    loop_index: Some(loop_index),
                });
                let mut nested = Vec::new();
                self.expand(child_definition, &mut nested)?;
                self.path.pop();
                for mut item in nested {
                    let local_start = loop_start
                        .checked_add(item.note.start.ticks())
                        .ok_or_else(arithmetic_overflow)?;
                    if local_start >= child.length.ticks() {
                        continue;
                    }
                    item.note.start = child.start + Tick::from_ticks(local_start);
                    item.note.duration = Tick::from_ticks(
                        item.note
                            .duration
                            .ticks()
                            .min(child.length.ticks() - local_start),
                    );
                    output.push(item);
                }
                loop_start = loop_start
                    .checked_add(child_definition.length.ticks())
                    .ok_or_else(arithmetic_overflow)?;
                loop_index = loop_index.checked_add(1).ok_or_else(arithmetic_overflow)?;
            }
        }
        Ok(())
    }

    fn consume_work(&mut self) -> Result<(), CoreError> {
        self.work.consume(1)
    }

    fn nested_note_id(
        &self,
        leaf_definition: &DefinitionId,
        leaf_note: &NoteId,
    ) -> Result<NoteId, CoreError> {
        let identity = PatternIdentity {
            root_definition_id: self.root.as_str(),
            path: self
                .path
                .iter()
                .filter_map(|entry| {
                    entry
                        .placement_id
                        .as_ref()
                        .zip(entry.loop_index)
                        .map(|(id, index)| (id.as_str(), index))
                })
                .collect(),
            leaf_definition_id: leaf_definition.as_str(),
            leaf_note_id: leaf_note.as_str(),
        };
        let encoded = serde_json::to_vec(&identity)
            .map_err(|error| CoreError::Serialization(error.to_string()))?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(NOTE_ID_DOMAIN);
        hasher.update(&encoded);
        Ok(format!("nested:{}", hasher.finalize().to_hex()).into())
    }
}

fn arithmetic_overflow() -> CoreError {
    CoreError::InvalidProject("definition composition arithmetic overflow".into())
}
