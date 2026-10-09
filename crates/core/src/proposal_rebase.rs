use crate::error::CoreError;
use crate::ids::HunkId;
use crate::music::Operation;
use crate::project::Project;
use crate::proposal_model::{HunkAcceptanceResult, Proposal, RebaseMode};
use crate::protocol::{Diagnostic, Severity};
use crate::validation::validate_operations;
use std::collections::BTreeSet;

pub struct RebaseAssessment {
    pub results: Vec<HunkAcceptanceResult>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn assess(
    project: &Project,
    proposal: &Proposal,
    selected: &BTreeSet<HunkId>,
    fingerprint_changed: bool,
) -> Result<RebaseAssessment, CoreError> {
    assess_metered(
        project,
        proposal,
        selected,
        fingerprint_changed,
        &mut crate::definition_composition::WorkMeter::new(),
    )
}

pub fn assess_metered(
    project: &Project,
    proposal: &Proposal,
    selected: &BTreeSet<HunkId>,
    fingerprint_changed: bool,
    work: &mut crate::definition_composition::WorkMeter,
) -> Result<RebaseAssessment, CoreError> {
    let instance = project
        .instances
        .iter()
        .find(|instance| instance.id == proposal.instance_id);
    let mut candidate_operations = instance
        .map(|item| item.overrides.clone())
        .unwrap_or_default();
    let current_definition = instance.and_then(|item| {
        project
            .definitions
            .iter()
            .find(|definition| definition.id == item.definition_id)
    });
    let current_notes = current_definition
        .map(|definition| crate::definition_composition::compose_metered(project, definition, work))
        .transpose()?
        .map(|notes| notes.into_iter().map(|item| item.note).collect::<Vec<_>>());
    let mut results = Vec::new();
    let mut diagnostics = Vec::new();
    for hunk in proposal
        .hunks
        .iter()
        .filter(|hunk| selected.contains(&hunk.id))
    {
        let conflict = match (&proposal.base_anchor, instance, current_definition) {
            (None, _, _) if fingerprint_changed => {
                Some(("RebaseConflict", "proposal has no rebase anchor"))
            }
            (_, None, _) => Some(("RebaseConflict", "target instance was deleted")),
            (Some(anchor), Some(target), _) if target.definition_id != anchor.definition_id => {
                Some(("TargetsChanged", "target instance changed definition"))
            }
            (_, _, None) => Some(("TargetsChanged", "source definition was deleted")),
            (Some(anchor), _, Some(definition)) => {
                let current_revisions =
                    crate::definition_graph::definition_revisions(project, &definition.id);
                match current_revisions {
                    Ok(current)
                        if crate::definition_graph::revision_closure_matches(
                            anchor.definition_revisions.as_deref(),
                            &current,
                        ) =>
                    {
                        ConflictContext {
                            definition_revision: anchor.definition_revision,
                            definition_changed: definition.revision != anchor.definition_revision,
                            definition,
                            composed_notes: current_notes.as_deref(),
                            existing_operations: &candidate_operations,
                            current_revisions: &current,
                        }
                        .operation_conflict(&hunk.operation, hunk.slot_binding.as_ref())
                    }
                    Ok(_) | Err(_) => {
                        Some(("TargetsChanged", "definition revision closure changed"))
                    }
                }
            }
            (None, Some(_), Some(_)) => None,
        };
        if let Some((code, message)) = conflict {
            results.push(HunkAcceptanceResult {
                hunk_id: hunk.id.clone(),
                applied: false,
                code: code.to_owned(),
            });
            diagnostics.push(Diagnostic {
                code: code.to_owned(),
                severity: Severity::Blocking,
                message: message.to_owned(),
                entity_id: Some(hunk.id.to_string()),
            });
        } else {
            candidate_operations.push(hunk.operation.clone());
            results.push(HunkAcceptanceResult {
                hunk_id: hunk.id.clone(),
                applied: true,
                code: if fingerprint_changed {
                    "Rebased".to_owned()
                } else {
                    "Applied".to_owned()
                },
            });
        }
    }
    Ok(RebaseAssessment {
        results,
        diagnostics,
    })
}

struct ConflictContext<'a> {
    definition_revision: u64,
    definition_changed: bool,
    definition: &'a crate::music::Definition,
    composed_notes: Option<&'a [crate::music::Note]>,
    existing_operations: &'a [Operation],
    current_revisions: &'a [crate::definition_graph::DefinitionRevision],
}

impl ConflictContext<'_> {
    fn operation_conflict<'a>(
        &self,
        operation: &Operation,
        slot_binding: Option<&crate::proposal_model::SlotBinding>,
    ) -> Option<(&'a str, &'a str)> {
        match operation {
            Operation::PatchNotes(_) => {
                return Some((
                    "UnsupportedCapability",
                    "note patches are instance-local edits",
                ));
            }
            Operation::ReplaceSlot { slot, .. } => {
                let Some(binding) = slot_binding else {
                    return Some(("AmbiguousTarget", "slot replacement has no binding"));
                };
                let Some(notes) = self.composed_notes else {
                    return Some(("TargetsChanged", "definition composition is invalid"));
                };
                let Some(selected) =
                    crate::modifier_binding::select_slot_notes(self.definition, slot, notes)
                else {
                    return Some(("TargetsChanged", "bound note slot was removed"));
                };
                let current_ids = selected.note_ids;
                if !crate::definition_graph::revision_closure_matches(
                    binding.definition_revisions.as_deref(),
                    self.current_revisions,
                ) || binding.slot != *slot
                    || binding.note_ids != current_ids
                    || binding.cardinality != current_ids.len()
                    || self.definition.revision
                        != binding.definition_revision.max(self.definition_revision)
                {
                    return match binding.mode {
                        RebaseMode::Strong => {
                            Some(("TargetsChanged", "strong-bound note targets changed"))
                        }
                        RebaseMode::Adaptive if binding.cardinality != current_ids.len() => {
                            Some(("AmbiguousTarget", "adaptive target cardinality changed"))
                        }
                        RebaseMode::Adaptive => {
                            Some(("TargetsChanged", "adaptive target resolution changed"))
                        }
                    };
                }
            }
            Operation::AddNotes { notes } => {
                let current_ids: BTreeSet<_> = self
                    .composed_notes
                    .unwrap_or(&self.definition.notes)
                    .iter()
                    .map(|note| &note.id)
                    .collect();
                if self.definition_changed
                    && notes.iter().any(|note| current_ids.contains(&note.id))
                {
                    return Some((
                        "TargetsChanged",
                        "added note id now exists in the source definition",
                    ));
                }
            }
            Operation::Transpose { .. }
            | Operation::Velocity { .. }
            | Operation::Gain { .. }
            | Operation::SetParameter { .. } => {}
        }
        let mut combined = self.existing_operations.to_vec();
        combined.push(operation.clone());
        if validate_operations(&combined).is_err() {
            Some((
                "ModifierConflict",
                "operation conflicts with current instance overrides",
            ))
        } else {
            None
        }
    }
}
