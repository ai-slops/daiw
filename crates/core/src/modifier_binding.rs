use crate::error::{BindingMismatch, CoreError, ModifierBindingIssue};
use crate::ids::{DefinitionId, NoteId};
use crate::music::{Definition, Modifier, NoteSlot, Operation};
use crate::time::Tick;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModifierBinding {
    pub definition_id: DefinitionId,
    pub definition_revision: u64,
    pub slots: BTreeMap<String, BoundSlot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub definition_revisions: Option<Vec<crate::definition_graph::DefinitionRevision>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoundSlot {
    pub range: NoteSlot,
    pub note_ids: Vec<NoteId>,
    pub cardinality: usize,
}

pub fn select_slot(definition: &Definition, slot: &str) -> Option<BoundSlot> {
    select_slot_notes(definition, slot, &definition.notes)
}

pub fn select_slot_notes(
    definition: &Definition,
    slot: &str,
    notes: &[crate::music::Note],
) -> Option<BoundSlot> {
    select_slot_iter(definition, slot, notes.iter())
}

pub fn select_slot_composed(
    definition: &Definition,
    slot: &str,
    notes: &[crate::definition_composition::ComposedNote],
) -> Option<BoundSlot> {
    select_slot_iter(definition, slot, notes.iter().map(|item| &item.note))
}

fn select_slot_iter<'a>(
    definition: &Definition,
    slot: &str,
    notes: impl Iterator<Item = &'a crate::music::Note>,
) -> Option<BoundSlot> {
    let range = if slot == "notes" {
        NoteSlot {
            start: Tick::default(),
            end: definition.length,
        }
    } else {
        definition.slots.get(slot)?.clone()
    };
    let note_ids: Vec<_> = notes
        .filter(|note| slot == "notes" || (note.start >= range.start && note.start < range.end))
        .map(|note| note.id.clone())
        .collect();
    Some(BoundSlot {
        cardinality: note_ids.len(),
        note_ids,
        range,
    })
}

pub fn capture(
    project: &crate::project::Project,
    definition: &Definition,
    modifier: &Modifier,
) -> Result<Option<ModifierBinding>, CoreError> {
    let composed = crate::definition_composition::compose(project, definition)?;
    let mut slots = BTreeMap::new();
    for operation in &modifier.operations {
        if let Operation::ReplaceSlot { slot, notes } = operation {
            let selected = select_slot_composed(definition, slot, &composed)
                .ok_or_else(|| CoreError::InvalidProject(format!("unknown note slot {slot}")))?;
            if notes.iter().any(|note| {
                note.start < selected.range.start
                    || note.start >= selected.range.end
                    || note.start + note.duration > selected.range.end
            }) {
                return Err(CoreError::InvalidProject(format!(
                    "replacement notes must fit slot {slot}"
                )));
            }
            if slots.insert(slot.clone(), selected).is_some() {
                return Err(CoreError::ModifierConflict(format!("slot:{slot}")));
            }
        }
    }
    if slots.is_empty() {
        Ok(None)
    } else {
        Ok(Some(ModifierBinding {
            definition_id: definition.id.clone(),
            definition_revision: definition.revision,
            slots,
            definition_revisions: crate::definition_graph::nested_revisions(
                project,
                &definition.id,
            )?,
        }))
    }
}

pub struct BindingCheck<'a> {
    pub project: &'a crate::project::Project,
    pub definition: &'a Definition,
    pub modifier: &'a Modifier,
    pub composed: &'a [crate::definition_composition::ComposedNote],
}

impl ModifierBinding {
    pub fn check(&self, context: BindingCheck<'_>) -> Result<(), CoreError> {
        if self.definition_id != context.definition.id {
            return Err(drift(
                context.modifier,
                BindingMismatch::DefinitionChanged,
                None,
            ));
        }
        let selectors: BTreeSet<_> = context
            .modifier
            .operations
            .iter()
            .filter_map(|operation| match operation {
                Operation::ReplaceSlot { slot, .. } => Some(slot),
                Operation::Transpose { .. }
                | Operation::Velocity { .. }
                | Operation::Gain { .. }
                | Operation::AddNotes { .. }
                | Operation::PatchNotes(_)
                | Operation::SetParameter { .. } => None,
            })
            .collect();
        if selectors != self.slots.keys().collect() {
            return Err(drift(
                context.modifier,
                BindingMismatch::SelectorsChanged,
                None,
            ));
        }
        for (slot, stored) in &self.slots {
            let current = select_slot_composed(context.definition, slot, context.composed)
                .ok_or_else(|| drift(context.modifier, BindingMismatch::SlotMissing, Some(slot)))?;
            if current.range != stored.range {
                return Err(drift(
                    context.modifier,
                    BindingMismatch::RangeChanged,
                    Some(slot),
                ));
            }
            if current.cardinality != stored.cardinality {
                return Err(drift(
                    context.modifier,
                    BindingMismatch::CardinalityChanged,
                    Some(slot),
                ));
            }
            if current.note_ids != stored.note_ids {
                return Err(drift(
                    context.modifier,
                    BindingMismatch::TargetsChanged,
                    Some(slot),
                ));
            }
        }
        let current_revisions =
            crate::definition_graph::definition_revisions(context.project, &context.definition.id)?;
        if !crate::definition_graph::revision_closure_matches(
            self.definition_revisions.as_deref(),
            &current_revisions,
        ) {
            return Err(drift(
                context.modifier,
                BindingMismatch::SourceRevisionChanged,
                None,
            ));
        }
        if self.definition_revision != context.definition.revision {
            return Err(drift(
                context.modifier,
                BindingMismatch::SourceRevisionChanged,
                None,
            ));
        }
        Ok(())
    }
}

fn drift(modifier: &Modifier, reason: BindingMismatch, slot: Option<&str>) -> CoreError {
    ModifierBindingIssue {
        modifier_id: modifier.id.clone(),
        reason,
        slot: slot.map(str::to_owned),
    }
    .into()
}
