use crate::command::EditScope;
use crate::error::CoreError;
use crate::ids::{InstanceId, ModifierId};
use crate::music::{Instance, Note, Operation};
use crate::project::Project;
use crate::validation::validate_notes;

pub fn edit_notes(
    project: &mut Project,
    instance_id: InstanceId,
    scope: EditScope,
    notes: Vec<Note>,
) -> Result<String, CoreError> {
    validate_notes(&notes)?;
    let definition_id = project
        .instances
        .iter()
        .find(|instance| instance.id == instance_id)
        .map(|instance| instance.definition_id.clone())
        .ok_or_else(|| CoreError::NotFound(instance_id.to_string()))?;
    match scope {
        EditScope::Source => {
            let definition = project
                .definitions
                .iter_mut()
                .find(|item| item.id == definition_id)
                .ok_or_else(|| CoreError::NotFound(definition_id.to_string()))?;
            definition.notes = notes;
            definition.revision += 1;
        }
        EditScope::Instance => {
            let instance = find_instance_mut(project, &instance_id)?;
            instance.overrides.retain(|operation| {
                !matches!(operation, Operation::ReplaceSlot { slot, .. } if slot == "notes")
            });
            instance.overrides.push(Operation::ReplaceSlot {
                slot: "notes".to_owned(),
                notes,
            });
        }
    }
    Ok("Edited notes".to_owned())
}

pub fn apply_modifier(
    project: &mut Project,
    instance_id: InstanceId,
    modifier_id: ModifierId,
) -> Result<String, CoreError> {
    if !project
        .modifiers
        .iter()
        .any(|modifier| modifier.id == modifier_id)
    {
        return Err(CoreError::NotFound(modifier_id.to_string()));
    }
    let instance = find_instance_mut(project, &instance_id)?;
    if !instance.modifiers.contains(&modifier_id) {
        instance.modifiers.push(modifier_id);
    }
    Ok("Applied modifier".to_owned())
}

pub fn find_instance_mut<'a>(
    project: &'a mut Project,
    id: &InstanceId,
) -> Result<&'a mut Instance, CoreError> {
    project
        .instances
        .iter_mut()
        .find(|instance| instance.id == *id)
        .ok_or_else(|| CoreError::NotFound(id.to_string()))
}
