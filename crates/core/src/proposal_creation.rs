use crate::error::CoreError;
use crate::ids::{HunkId, ProposalId};
use crate::music::Operation;
use crate::project::Project;
use crate::proposal_model::{
    Hunk, Proposal, ProposalAnchor, ProposalParams, ProposalStatus, SlotBinding,
};

pub(crate) fn create(
    project: &Project,
    params: ProposalParams,
    session: &mut crate::composition_cache::ResolutionSession<'_>,
) -> Result<Proposal, CoreError> {
    crate::revision::require_revision(project, &params.expected_revision)?;
    let instance = project
        .instances
        .iter()
        .find(|instance| instance.id == params.instance_id)
        .ok_or_else(|| CoreError::NotFound(params.instance_id.to_string()))?;
    let definition = project
        .definitions
        .iter()
        .find(|definition| definition.id == instance.definition_id)
        .ok_or_else(|| CoreError::NotFound(instance.definition_id.to_string()))?;
    let operations = match params.operations {
        Some(operations) => operations,
        None => crate::proposal_adapter::parse_request(&params.request)?,
    };
    if operations.is_empty() {
        return Err(CoreError::UnsupportedCapability(params.request));
    }
    crate::operation_validation::validate_reusable_operations(&operations)?;
    let composed_notes = session
        .compose(
            project,
            definition,
            &mut crate::definition_composition::WorkMeter::new(),
        )?
        .into_iter()
        .map(|item| item.note)
        .collect::<Vec<_>>();
    let definition_revisions = crate::definition_graph::nested_revisions(project, &definition.id)?;
    let context = HunkContext {
        definition,
        composed_notes: &composed_notes,
        definition_revisions: &definition_revisions,
    };
    let hunks = operations
        .into_iter()
        .enumerate()
        .map(|(index, operation)| context.hunk(index, operation))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Proposal {
        id: ProposalId::new_random(),
        name: "Agent proposal".to_owned(),
        request: params.request,
        base_revision: project.revision.clone(),
        instance_id: params.instance_id,
        status: ProposalStatus::Pending,
        hunks,
        base_anchor: Some(ProposalAnchor {
            definition_id: definition.id.clone(),
            definition_revision: definition.revision,
            note_ids: composed_notes.iter().map(|note| note.id.clone()).collect(),
            definition_revisions,
        }),
        base_fingerprint: Some(crate::revision::musical_fingerprint(project)?),
    })
}

struct HunkContext<'a> {
    definition: &'a crate::music::Definition,
    composed_notes: &'a [crate::music::Note],
    definition_revisions: &'a Option<Vec<crate::definition_graph::DefinitionRevision>>,
}

impl HunkContext<'_> {
    fn hunk(&self, index: usize, operation: Operation) -> Result<Hunk, CoreError> {
        let slot_binding = match &operation {
            Operation::ReplaceSlot { slot, .. } => {
                let selected = crate::modifier_binding::select_slot_notes(
                    self.definition,
                    slot,
                    self.composed_notes,
                )
                .ok_or_else(|| CoreError::InvalidProject(format!("unknown note slot {slot}")))?;
                let note_ids = selected.note_ids;
                Some(SlotBinding {
                    slot: slot.clone(),
                    cardinality: note_ids.len(),
                    note_ids,
                    definition_revision: self.definition.revision,
                    mode: crate::proposal_model::RebaseMode::Strong,
                    definition_revisions: self.definition_revisions.clone(),
                })
            }
            Operation::Transpose { .. }
            | Operation::Velocity { .. }
            | Operation::Gain { .. }
            | Operation::AddNotes { .. }
            | Operation::PatchNotes(_)
            | Operation::SetParameter { .. } => None,
        };
        Ok(Hunk {
            id: HunkId::new_random(),
            title: format!("Change {}", index + 1),
            description: crate::proposal_adapter::operation_description(&operation).to_owned(),
            operation,
            dependencies: Vec::new(),
            accepted: false,
            slot_binding,
        })
    }
}
