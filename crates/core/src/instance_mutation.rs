use crate::command::EditScope;
use crate::error::CoreError;
use crate::ids::{InstanceId, ModifierId};
use crate::music::{Instance, Note, Operation};
use crate::note_patch::NotePatch;
use crate::project::Project;
use crate::validation::validate_notes;

pub(crate) fn edit_notes_with_session(
    project: &mut Project,
    instance_id: InstanceId,
    scope: EditScope,
    notes: Vec<Note>,
    session: &mut crate::composition_cache::ResolutionSession<'_>,
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
            return crate::definition_mutation::edit_notes_legacy(project, definition_id, notes);
        }
        EditScope::Instance => {
            let definition_length = project
                .definitions
                .iter()
                .find(|definition| definition.id == definition_id)
                .map(|definition| definition.length)
                .ok_or_else(|| CoreError::NotFound(definition_id.to_string()))?;
            let instance = find_instance_mut(project, &instance_id)?;
            crate::note_patch::validate_bounds(&notes, definition_length.min(instance.length))?;
            instance
                .overrides
                .retain(|operation| !matches!(operation, Operation::PatchNotes(_)));
            let before = crate::resolver::editable_notes_metered_with_session(
                project,
                &instance_id,
                &mut crate::definition_composition::WorkMeter::new(),
                session,
            )?;
            if let Some(patch) = NotePatch::between(&before, notes) {
                find_instance_mut(project, &instance_id)?
                    .overrides
                    .push(Operation::PatchNotes(patch));
            }
        }
    }
    Ok("Edited notes".to_owned())
}

pub(crate) fn apply_modifier_with_session(
    project: &mut Project,
    instance_id: InstanceId,
    modifier_id: ModifierId,
    session: &mut crate::composition_cache::ResolutionSession<'_>,
) -> Result<String, CoreError> {
    if !project
        .modifiers
        .iter()
        .any(|modifier| modifier.id == modifier_id)
    {
        return Err(CoreError::NotFound(modifier_id.to_string()));
    }
    let instance = find_instance_mut(project, &instance_id)?;
    if instance.modifiers.contains(&modifier_id) {
        return Ok("Modifier already applied".to_owned());
    }
    let binding = capture_binding(project, &instance_id, &modifier_id, session)?;
    let instance = find_instance_mut(project, &instance_id)?;
    if let Some(binding) = binding {
        instance
            .modifier_bindings
            .insert(modifier_id.clone(), binding);
    }
    instance.modifiers.push(modifier_id);
    Ok("Applied modifier".to_owned())
}

pub(crate) fn rebase_modifier_with_session(
    project: &mut Project,
    instance_id: InstanceId,
    modifier_id: ModifierId,
    session: &mut crate::composition_cache::ResolutionSession<'_>,
) -> Result<String, CoreError> {
    if !find_instance_mut(project, &instance_id)?
        .modifiers
        .contains(&modifier_id)
    {
        return Err(CoreError::NotFound(format!(
            "modifier attachment {modifier_id}"
        )));
    }
    let binding =
        capture_binding(project, &instance_id, &modifier_id, session)?.ok_or_else(|| {
            CoreError::UnsupportedCapability("modifier has no slot selectors".to_owned())
        })?;
    find_instance_mut(project, &instance_id)?
        .modifier_bindings
        .insert(modifier_id, binding);
    Ok("Rebased modifier binding".to_owned())
}

fn capture_binding(
    project: &Project,
    instance_id: &InstanceId,
    modifier_id: &ModifierId,
    session: &mut crate::composition_cache::ResolutionSession<'_>,
) -> Result<Option<crate::modifier_binding::ModifierBinding>, CoreError> {
    let instance = project
        .instances
        .iter()
        .find(|instance| instance.id == *instance_id)
        .ok_or_else(|| CoreError::NotFound(instance_id.to_string()))?;
    let definition = project
        .definitions
        .iter()
        .find(|definition| definition.id == instance.definition_id)
        .ok_or_else(|| CoreError::NotFound(instance.definition_id.to_string()))?;
    let modifier = project
        .modifiers
        .iter()
        .find(|modifier| modifier.id == *modifier_id)
        .ok_or_else(|| CoreError::NotFound(modifier_id.to_string()))?;
    crate::modifier_binding::capture_with_session(project, definition, modifier, session)
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
