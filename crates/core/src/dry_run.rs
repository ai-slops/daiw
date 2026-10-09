use crate::change_impact::{ChangeImpact, analyze};
use crate::command::Command;
use crate::engine::{affected_instances, preview_change};
use crate::ids::InstanceId;
use crate::project::Project;
use crate::protocol::{Diagnostic, Severity};
use crate::rebase_report::{InstanceCheck, InstanceRebaseReport, NoteResolutionReport, check};
use crate::request_params::ChangeParams;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DryRunResult {
    executable: bool,
    affected_instances: Vec<InstanceId>,
    hunks: Vec<CommandHunk>,
    diagnostics: Vec<Diagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    impact: Option<ChangeImpact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    note_resolution: Option<NoteResolutionReport>,
}

#[derive(Serialize)]
struct CommandHunk {
    command: Command,
}

pub fn inspect(project: &Project, params: ChangeParams) -> DryRunResult {
    let requested = affected_instances(project, &params.command);
    match preview_change(project, &params.expected_revision, params.command.clone()) {
        Ok(candidate) => {
            let mut work = crate::definition_composition::WorkMeter::new();
            let (affected, impact) = analyze(project, &candidate, &requested);
            let mut diagnostics = Vec::new();
            let instances = affected
                .iter()
                .map(|id| {
                    let before = check(project, id, &mut work);
                    let after = check(&candidate, id, &mut work);
                    if let InstanceCheck::Conflict { diagnostic, .. } = &after {
                        diagnostics.push(diagnostic.clone());
                    }
                    InstanceRebaseReport {
                        instance_id: id.clone(),
                        before,
                        after,
                    }
                })
                .collect();
            let ready = diagnostics.is_empty();
            DryRunResult {
                executable: true,
                affected_instances: affected,
                hunks: vec![CommandHunk {
                    command: params.command,
                }],
                diagnostics,
                impact: Some(impact),
                note_resolution: Some(NoteResolutionReport { ready, instances }),
            }
        }
        Err(error) => DryRunResult {
            executable: false,
            affected_instances: requested,
            hunks: Vec::new(),
            diagnostics: vec![Diagnostic {
                code: error.code().to_owned(),
                severity: Severity::Blocking,
                message: error.to_string(),
                entity_id: None,
            }],
            impact: None,
            note_resolution: None,
        },
    }
}
