use crate::command::{ChangeRequest, Command, EditScope};
use crate::error::CoreError;
use crate::ids::{InstanceId, Revision};
use crate::project::Project;
use crate::protocol::Principal;
use crate::revision::{advance, require_revision};
use crate::validation::validate_project;

pub fn apply_change(
    project: &Project,
    principal: Principal,
    request: ChangeRequest,
) -> Result<Project, CoreError> {
    let mut cache = crate::composition_cache::uncached_session();
    apply_change_with_session(
        project,
        principal,
        request,
        &mut crate::composition_cache::ResolutionSession::new(&mut cache),
    )
}

pub(crate) fn apply_change_with_session(
    project: &Project,
    principal: Principal,
    request: ChangeRequest,
    session: &mut crate::composition_cache::ResolutionSession<'_>,
) -> Result<Project, CoreError> {
    if principal != Principal::Human {
        return Err(CoreError::PermissionDenied);
    }
    require_revision(project, &request.expected_revision)?;
    let mut changed = project.clone();
    let message =
        crate::mutation::apply_command_with_session(&mut changed, request.command, session)?;
    validate_project(&changed)?;
    advance(changed, "human", &message)
}

pub fn preview_change(
    project: &Project,
    expected: &Revision,
    command: Command,
) -> Result<Project, CoreError> {
    let mut cache = crate::composition_cache::uncached_session();
    preview_change_with_session(
        project,
        expected,
        command,
        &mut crate::composition_cache::ResolutionSession::new(&mut cache),
    )
}

pub(crate) fn preview_change_with_session(
    project: &Project,
    expected: &Revision,
    command: Command,
    session: &mut crate::composition_cache::ResolutionSession<'_>,
) -> Result<Project, CoreError> {
    require_revision(project, expected)?;
    let mut changed = project.clone();
    crate::mutation::apply_command_with_session(&mut changed, command, session)?;
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
                crate::definition_graph::dependent_instances(project, &target.definition_id)
            })
            .unwrap_or_default(),
        Command::EditDefinitionNotes { definition_id, .. } => {
            crate::definition_graph::dependent_instances(project, definition_id)
        }
        Command::AddDefinitionChild {
            parent_definition_id,
            ..
        }
        | Command::SetDefinitionChild {
            parent_definition_id,
            ..
        }
        | Command::RemoveDefinitionChild {
            parent_definition_id,
            ..
        } => crate::definition_graph::dependent_instances(project, parent_definition_id),
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
