use crate::command::{ChangeRequest, Command, EditScope};
use crate::error::CoreError;
use crate::ids::{InstanceId, Revision};
use crate::mutation::apply_command;
use crate::project::Project;
use crate::protocol::Principal;
use crate::revision::{advance, require_revision};
use crate::validation::validate_project;

pub fn apply_change(
    project: &Project,
    principal: Principal,
    request: ChangeRequest,
) -> Result<Project, CoreError> {
    if principal != Principal::Human {
        return Err(CoreError::PermissionDenied);
    }
    require_revision(project, &request.expected_revision)?;
    let mut changed = project.clone();
    let message = apply_command(&mut changed, request.command)?;
    validate_project(&changed)?;
    advance(changed, "human", &message)
}

pub fn preview_change(
    project: &Project,
    expected: &Revision,
    command: Command,
) -> Result<Project, CoreError> {
    require_revision(project, expected)?;
    let mut changed = project.clone();
    apply_command(&mut changed, command)?;
    validate_project(&changed)?;
    Ok(changed)
}

#[must_use]
pub fn affected_instances(project: &Project, command: &Command) -> Vec<InstanceId> {
    match command {
        Command::EditNotes {
            instance_id,
            scope: EditScope::Source,
            ..
        }
        | Command::TransformSelectedNotes {
            selection:
                crate::note_selection::SelectionHandle {
                    instance_id,
                    scope: EditScope::Source,
                    ..
                },
            ..
        } => project
            .instances
            .iter()
            .find(|instance| instance.id == *instance_id)
            .map(|target| {
                project
                    .instances
                    .iter()
                    .filter(|item| item.definition_id == target.definition_id)
                    .map(|item| item.id.clone())
                    .collect()
            })
            .unwrap_or_default(),
        Command::MoveInstance { instance_id, .. }
        | Command::DuplicateInstance { instance_id }
        | Command::DeleteInstance { instance_id }
        | Command::EditNotes {
            instance_id,
            scope: EditScope::Instance,
            ..
        }
        | Command::TransformSelectedNotes {
            selection:
                crate::note_selection::SelectionHandle {
                    instance_id,
                    scope: EditScope::Instance,
                    ..
                },
            ..
        }
        | Command::ApplyModifier { instance_id, .. }
        | Command::RebaseModifier { instance_id, .. }
        | Command::SetModifierOrdering { instance_id, .. }
        | Command::RemoveModifier { instance_id, .. } => vec![instance_id.clone()],
        Command::SetTempo { .. }
        | Command::SetTempoMap { .. }
        | Command::SetMeter { .. }
        | Command::SetMeterMap { .. } => project
            .instances
            .iter()
            .map(|instance| instance.id.clone())
            .collect(),
        Command::RenameProject { .. }
        | Command::AddTrack { .. }
        | Command::SetTrack { .. }
        | Command::AddDefinition { .. }
        | Command::Instantiate { .. }
        | Command::CreateModifier { .. }
        | Command::AddAudioRegion { .. }
        | Command::RemoveAudioRegion { .. } => Vec::new(),
    }
}
