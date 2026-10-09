use crate::command::EditScope;
use crate::error::CoreError;
use crate::ids::{DefinitionId, InstanceId, NoteId, ProjectId, Revision};
use crate::music::{Definition, Note};
pub use crate::note_selector::NoteSelector;
use crate::project::Project;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_SELECTION_RESULTS: usize = 4_096;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryNotesParams {
    pub instance_id: InstanceId,
    pub scope: EditScope,
    pub selector: NoteSelector,
    pub max_results: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionHandle {
    pub project_id: ProjectId,
    pub base_revision: Revision,
    pub definition_id: DefinitionId,
    pub definition_revision: u64,
    pub instance_id: InstanceId,
    pub scope: EditScope,
    pub selector: NoteSelector,
    pub note_ids: Vec<NoteId>,
    pub cardinality: usize,
    pub resolution_fingerprint: String,
}

pub fn query_notes(
    project: &Project,
    params: QueryNotesParams,
) -> Result<SelectionHandle, CoreError> {
    let mut cache = crate::composition_cache::uncached_session();
    query_notes_with_session(
        project,
        params,
        &mut crate::composition_cache::ResolutionSession::new(&mut cache),
    )
}

pub(crate) fn query_notes_with_session(
    project: &Project,
    params: QueryNotesParams,
    session: &mut crate::composition_cache::ResolutionSession<'_>,
) -> Result<SelectionHandle, CoreError> {
    validate_limit(params.max_results)?;
    let selector = crate::note_selector::normalize(params.selector)?;
    let (definition, notes) =
        editable_context(project, &params.instance_id, params.scope, session)?;
    let note_ids = crate::note_selector::resolve(&notes, definition, &selector)?;
    if note_ids.len() > params.max_results {
        return Err(CoreError::SelectionLimitExceeded {
            limit: params.max_results,
            actual: note_ids.len(),
        });
    }
    let mut handle = SelectionHandle {
        project_id: project.id.clone(),
        base_revision: project.revision.clone(),
        definition_id: definition.id.clone(),
        definition_revision: definition.revision,
        instance_id: params.instance_id,
        scope: params.scope,
        selector,
        cardinality: note_ids.len(),
        note_ids,
        resolution_fingerprint: String::new(),
    };
    handle.resolution_fingerprint = fingerprint(&handle)?;
    Ok(handle)
}

pub(crate) fn validate_handle_with_session(
    project: &Project,
    handle: &SelectionHandle,
    session: &mut crate::composition_cache::ResolutionSession<'_>,
) -> Result<Vec<Note>, CoreError> {
    if handle.project_id != project.id || handle.base_revision != project.revision {
        return Err(CoreError::SelectionStale);
    }
    if handle.cardinality > MAX_SELECTION_RESULTS
        || handle.cardinality != handle.note_ids.len()
        || handle.note_ids.iter().collect::<BTreeSet<_>>().len() != handle.note_ids.len()
        || fingerprint(handle)? != handle.resolution_fingerprint
    {
        return Err(CoreError::SelectionInvalid(
            "selection proof does not match its contents".into(),
        ));
    }
    let (definition, notes) =
        editable_context(project, &handle.instance_id, handle.scope, session)?;
    if definition.id != handle.definition_id || definition.revision != handle.definition_revision {
        return Err(CoreError::SelectionStale);
    }
    let selector = crate::note_selector::normalize(handle.selector.clone())?;
    let current_ids = crate::note_selector::resolve(&notes, definition, &selector)?;
    if current_ids.len() != handle.cardinality || current_ids != handle.note_ids {
        return Err(CoreError::SelectionInvalid(
            "selector resolution changed".into(),
        ));
    }
    Ok(notes)
}

fn editable_context<'a>(
    project: &'a Project,
    instance_id: &InstanceId,
    scope: EditScope,
    session: &mut crate::composition_cache::ResolutionSession<'_>,
) -> Result<(&'a Definition, Vec<Note>), CoreError> {
    let instance = project
        .instances
        .iter()
        .find(|item| item.id == *instance_id)
        .ok_or_else(|| CoreError::NotFound(instance_id.to_string()))?;
    let definition = project
        .definitions
        .iter()
        .find(|item| item.id == instance.definition_id)
        .ok_or_else(|| CoreError::NotFound(instance.definition_id.to_string()))?;
    let notes = match scope {
        EditScope::Source => definition.notes.clone(),
        EditScope::Instance => crate::resolver::editable_notes_metered_with_session(
            project,
            instance_id,
            &mut crate::definition_composition::WorkMeter::new(),
            session,
        )?,
    };
    Ok((definition, notes))
}

fn validate_limit(limit: usize) -> Result<(), CoreError> {
    if (1..=MAX_SELECTION_RESULTS).contains(&limit) {
        Ok(())
    } else {
        Err(CoreError::InvalidParams(format!(
            "maxResults must be between 1 and {MAX_SELECTION_RESULTS}"
        )))
    }
}

fn fingerprint(handle: &SelectionHandle) -> Result<String, CoreError> {
    let mut proof = handle.clone();
    proof.resolution_fingerprint.clear();
    let encoded =
        serde_json::to_vec(&proof).map_err(|error| CoreError::Serialization(error.to_string()))?;
    Ok(blake3::hash(&encoded).to_hex().to_string())
}
