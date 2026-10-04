use crate::error::CoreError;
use crate::ids::ModifierId;
use crate::music::{Instance, Operation};
use crate::project::Project;
use crate::time::MAX_TICKS;
use std::collections::BTreeSet;

pub fn validate(instance: &Instance) -> Result<(), CoreError> {
    for (id, binding) in &instance.modifier_bindings {
        if !instance.modifiers.contains(id) || binding.slots.is_empty() {
            return Err(CoreError::InvalidProject(format!(
                "invalid modifier binding {id}"
            )));
        }
        for (name, slot) in &binding.slots {
            let unique: BTreeSet<_> = slot.note_ids.iter().collect();
            if name.trim().is_empty()
                || slot.range.start.ticks() < 0
                || slot.range.end <= slot.range.start
                || slot.range.end.ticks() > MAX_TICKS
                || slot.cardinality != slot.note_ids.len()
                || unique.len() != slot.note_ids.len()
            {
                return Err(CoreError::InvalidProject(format!(
                    "malformed modifier binding {id} slot {name}"
                )));
            }
        }
    }
    Ok(())
}

pub fn unbound_modifier_ids(project: &Project, instance: &Instance) -> Vec<ModifierId> {
    instance
        .modifiers
        .iter()
        .filter(|id| {
            !instance.modifier_bindings.contains_key(*id)
                && project.modifiers.iter().any(|modifier| {
                    modifier.id == **id
                        && modifier
                            .operations
                            .iter()
                            .any(|operation| matches!(operation, Operation::ReplaceSlot { .. }))
                })
        })
        .cloned()
        .collect()
}
