use crate::error::CoreError;
use crate::ids::{ChildPlacementId, DefinitionId};
use crate::music::{ChildPlacement, Definition, Note};
use crate::project::Project;

pub fn add_child(
    project: &mut Project,
    parent_id: DefinitionId,
    child: ChildPlacement,
) -> Result<String, CoreError> {
    require_definition(project, &child.definition_id)?;
    let parent = find_mut(project, &parent_id)?;
    let revision = next_revision(parent)?;
    parent.children.push(child);
    parent.revision = revision;
    Ok("Added definition child".into())
}

pub fn set_child(
    project: &mut Project,
    parent_id: DefinitionId,
    child: ChildPlacement,
) -> Result<String, CoreError> {
    require_definition(project, &child.definition_id)?;
    let parent = find_mut(project, &parent_id)?;
    let index = parent
        .children
        .iter()
        .position(|item| item.id == child.id)
        .ok_or_else(|| CoreError::NotFound(child.id.to_string()))?;
    if parent.children[index] == child {
        return Err(CoreError::InvalidParams(
            "setDefinitionChild must change the placement".into(),
        ));
    }
    let revision = next_revision(parent)?;
    parent.children[index] = child;
    parent.revision = revision;
    Ok("Updated definition child".into())
}

pub fn remove_child(
    project: &mut Project,
    parent_id: DefinitionId,
    child_id: ChildPlacementId,
) -> Result<String, CoreError> {
    let parent = find_mut(project, &parent_id)?;
    let index = parent
        .children
        .iter()
        .position(|child| child.id == child_id)
        .ok_or_else(|| CoreError::NotFound(child_id.to_string()))?;
    let revision = next_revision(parent)?;
    parent.children.remove(index);
    parent.revision = revision;
    Ok("Removed definition child".into())
}

pub fn edit_notes(
    project: &mut Project,
    definition_id: DefinitionId,
    notes: Vec<Note>,
) -> Result<String, CoreError> {
    crate::validation::validate_notes(&notes)?;
    let definition = find_mut(project, &definition_id)?;
    replace_notes(definition, notes, NoOpBehavior::Reject)?;
    Ok("Edited definition notes".into())
}

pub fn edit_notes_legacy(
    project: &mut Project,
    definition_id: DefinitionId,
    notes: Vec<Note>,
) -> Result<String, CoreError> {
    crate::validation::validate_notes(&notes)?;
    let definition = find_mut(project, &definition_id)?;
    replace_notes(definition, notes, NoOpBehavior::Accept)?;
    Ok("Edited notes".into())
}

fn require_definition(project: &Project, id: &DefinitionId) -> Result<(), CoreError> {
    project
        .definitions
        .iter()
        .any(|definition| definition.id == *id)
        .then_some(())
        .ok_or_else(|| CoreError::NotFound(id.to_string()))
}

fn find_mut<'a>(
    project: &'a mut Project,
    id: &DefinitionId,
) -> Result<&'a mut Definition, CoreError> {
    project
        .definitions
        .iter_mut()
        .find(|definition| definition.id == *id)
        .ok_or_else(|| CoreError::NotFound(id.to_string()))
}

fn next_revision(definition: &Definition) -> Result<u64, CoreError> {
    definition.revision.checked_add(1).ok_or_else(|| {
        CoreError::InvalidProject(format!("definition {} revision overflow", definition.id))
    })
}

enum NoOpBehavior {
    Accept,
    Reject,
}

fn replace_notes(
    definition: &mut Definition,
    notes: Vec<Note>,
    no_op: NoOpBehavior,
) -> Result<(), CoreError> {
    if definition.notes == notes {
        return match no_op {
            NoOpBehavior::Accept => Ok(()),
            NoOpBehavior::Reject => Err(CoreError::InvalidParams(
                "editDefinitionNotes must change notes".into(),
            )),
        };
    }
    let revision = next_revision(definition)?;
    definition.notes = notes;
    definition.revision = revision;
    Ok(())
}
