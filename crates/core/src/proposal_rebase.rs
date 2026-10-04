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
) -> RebaseAssessment {
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
            (Some(anchor), _, Some(definition)) => operation_conflict(
                &hunk.operation,
                hunk.slot_binding.as_ref(),
                anchor.definition_revision,
                definition,
                &candidate_operations,
            ),
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
    RebaseAssessment {
        results,
        diagnostics,
    }
}

fn operation_conflict<'a>(
    operation: &Operation,
    slot_binding: Option<&crate::proposal_model::SlotBinding>,
    definition_revision: u64,
    definition: &crate::music::Definition,
    existing_operations: &[Operation],
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
            let Some(selected) = crate::modifier_binding::select_slot(definition, slot) else {
                return Some(("TargetsChanged", "bound note slot was removed"));
            };
            let current_ids = selected.note_ids;
            if binding.slot != *slot
                || binding.note_ids != current_ids
                || binding.cardinality != current_ids.len()
                || definition.revision != binding.definition_revision.max(definition_revision)
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
            let current_ids: BTreeSet<_> = definition.notes.iter().map(|note| &note.id).collect();
            if notes.iter().any(|note| current_ids.contains(&note.id)) {
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
    let mut combined = existing_operations.to_vec();
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
