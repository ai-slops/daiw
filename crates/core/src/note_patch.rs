use crate::error::{CoreError, NoteTargetIssue};
use crate::ids::NoteId;
use crate::music::Note;
use crate::time::Tick;
use crate::validation::validate_notes;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotePatch {
    pub updates: Vec<Note>,
    pub added: Vec<Note>,
    pub removed: Vec<NoteId>,
}

pub fn validate_bounds(notes: &[Note], length: Tick) -> Result<(), CoreError> {
    if notes
        .iter()
        .any(|note| note.start >= length || note.start + note.duration > length)
    {
        return Err(CoreError::InvalidProject(
            "local notes must fit the editable pattern length".to_owned(),
        ));
    }
    Ok(())
}

impl NotePatch {
    pub fn between(before: &[Note], after: Vec<Note>) -> Option<Self> {
        let originals: BTreeMap<_, _> = before.iter().map(|note| (&note.id, note)).collect();
        let retained: BTreeSet<_> = after.iter().map(|note| &note.id).collect();
        let removed = before
            .iter()
            .filter(|note| !retained.contains(&note.id))
            .map(|note| note.id.clone())
            .collect();
        let mut patch = Self {
            updates: Vec::new(),
            added: Vec::new(),
            removed,
        };
        for note in after {
            match originals.get(&note.id) {
                Some(original) if **original == note => {}
                Some(_) => patch.updates.push(note),
                None => patch.added.push(note),
            }
        }
        if patch.updates.is_empty() && patch.added.is_empty() && patch.removed.is_empty() {
            None
        } else {
            Some(patch)
        }
    }

    pub fn validate(&self) -> Result<(), CoreError> {
        validate_notes(&self.updates)?;
        validate_notes(&self.added)?;
        let mut ids = BTreeSet::new();
        for id in self
            .updates
            .iter()
            .chain(&self.added)
            .map(|note| &note.id)
            .chain(&self.removed)
        {
            if !ids.insert(id) {
                return Err(CoreError::ModifierConflict(format!(
                    "note patch repeats target {id}"
                )));
            }
        }
        Ok(())
    }

    pub fn validate_targets<'a>(
        &self,
        notes: impl Iterator<Item = &'a NoteId>,
    ) -> Result<(), CoreError> {
        let mut ids = BTreeSet::new();
        for id in notes {
            if !ids.insert(id) {
                return Err(NoteTargetIssue::Ambiguous {
                    note_id: id.clone(),
                }
                .into());
            }
        }
        for id in self
            .updates
            .iter()
            .map(|note| &note.id)
            .chain(&self.removed)
        {
            if !ids.contains(id) {
                return Err(NoteTargetIssue::Missing {
                    note_id: id.clone(),
                }
                .into());
            }
        }
        for note in &self.added {
            if ids.contains(&note.id) {
                return Err(NoteTargetIssue::AddedIdCollision {
                    note_id: note.id.clone(),
                }
                .into());
            }
        }
        Ok(())
    }
}
