use crate::error::{CoreError, ModifierBindingIssue, NoteTargetIssue};
use crate::ids::{DefinitionId, InstanceId, ModifierId};
use crate::project::Project;
use crate::protocol::{Diagnostic, Severity};
use crate::resolver::editable_notes;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteResolutionReport {
    pub ready: bool,
    pub instances: Vec<InstanceRebaseReport>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceRebaseReport {
    pub instance_id: InstanceId,
    pub before: InstanceCheck,
    pub after: InstanceCheck,
}

#[derive(Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "status"
)]
pub enum InstanceCheck {
    Absent,
    Clean {
        definition_id: DefinitionId,
        definition_revision: u64,
        note_count: usize,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        unbound_modifier_ids: Vec<ModifierId>,
    },
    Conflict {
        definition_id: DefinitionId,
        definition_revision: Option<u64>,
        diagnostic: Diagnostic,
        #[serde(skip_serializing_if = "Option::is_none")]
        target: Option<NoteTargetIssue>,
        #[serde(skip_serializing_if = "Option::is_none")]
        binding: Option<ModifierBindingIssue>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        unbound_modifier_ids: Vec<ModifierId>,
        actions: Vec<ResolutionAction>,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ResolutionAction {
    ReviseCommand,
    RestoreMissingNote,
    EditInstanceNotes,
    ChooseUniqueNoteIds,
    ReviewModifiers,
    RebaseModifier,
}

pub fn check(project: &Project, id: &InstanceId) -> InstanceCheck {
    let Some(instance) = project.instances.iter().find(|instance| instance.id == *id) else {
        return InstanceCheck::Absent;
    };
    let definition = project
        .definitions
        .iter()
        .find(|definition| definition.id == instance.definition_id);
    let unbound_modifier_ids = crate::binding_validation::unbound_modifier_ids(project, instance);
    let result = definition
        .ok_or_else(|| CoreError::NotFound(instance.definition_id.to_string()))
        .and_then(|definition| editable_notes(project, id).map(|notes| (definition, notes.len())));
    match result {
        Ok((definition, count)) => InstanceCheck::Clean {
            definition_id: definition.id.clone(),
            definition_revision: definition.revision,
            note_count: count,
            unbound_modifier_ids,
        },
        Err(error) => {
            let (target, binding) = match &error {
                CoreError::NoteTarget(target) => (Some(target.clone()), None),
                CoreError::ModifierBinding(binding) => (None, Some(binding.clone())),
                CoreError::MissingState
                | CoreError::NativeFormat(_)
                | CoreError::UnexpectedState
                | CoreError::UnknownMethod(_)
                | CoreError::InvalidParams(_)
                | CoreError::InvalidProject(_)
                | CoreError::NotFound(_)
                | CoreError::StaleRevision { .. }
                | CoreError::PermissionDenied
                | CoreError::SelectionStale
                | CoreError::SelectionInvalid(_)
                | CoreError::SelectionLimitExceeded { .. }
                | CoreError::UnsupportedCapability(_)
                | CoreError::ModifierConflict(_)
                | CoreError::ModifierOrderingInvalid(_)
                | CoreError::ModifierOrderingCycle
                | CoreError::ModifierOrderRequired { .. }
                | CoreError::MissingDependency(_)
                | CoreError::StaleProposal
                | CoreError::ProposalClosed
                | CoreError::HunkAlreadyAccepted(_)
                | CoreError::RebaseConflict(_)
                | CoreError::Io(_)
                | CoreError::Serialization(_) => (None, None),
            };
            let actions = match &target {
                Some(NoteTargetIssue::Missing { .. }) => vec![
                    ResolutionAction::RestoreMissingNote,
                    ResolutionAction::EditInstanceNotes,
                ],
                Some(NoteTargetIssue::AddedIdCollision { .. }) => vec![
                    ResolutionAction::ChooseUniqueNoteIds,
                    ResolutionAction::EditInstanceNotes,
                ],
                Some(NoteTargetIssue::Ambiguous { .. }) => vec![ResolutionAction::ReviewModifiers],
                None => match &binding {
                    Some(_) => vec![ResolutionAction::RebaseModifier],
                    None => vec![ResolutionAction::ReviseCommand],
                },
            };
            InstanceCheck::Conflict {
                definition_id: instance.definition_id.clone(),
                definition_revision: definition.map(|definition| definition.revision),
                diagnostic: Diagnostic {
                    code: error.code().to_owned(),
                    severity: Severity::Blocking,
                    message: error.to_string(),
                    entity_id: Some(id.to_string()),
                },
                target,
                binding,
                unbound_modifier_ids,
                actions,
            }
        }
    }
}
