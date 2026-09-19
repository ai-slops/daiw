use crate::command::ChangeRequest;
use crate::engine::{affected_instances, apply_change, preview_change};
use crate::error::CoreError;
use crate::fixture::create_project;
use crate::project::Project;
use crate::proposal_model::{Acceptance, Rejection};
use crate::proposals::{accept_proposal, create_proposal, reject_proposal};
use crate::protocol::{Diagnostic, FailureResponse, Request, Response, Severity, SuccessResponse};
use crate::request_params::{
    AcceptParams, ChangeParams, CreateParams, ExplainParams, ImportParams, RejectParams,
    RestoreParams, SnapshotParams,
};
use crate::resolver::snapshot;
use crate::revision::{advance, canonical_hash, require_revision};
use crate::validation::validate_project;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

pub fn dispatch(request: Request) -> Response {
    let id = request.id.clone();
    match execute(request) {
        Ok((state, result)) => Response::Success(Box::new(SuccessResponse {
            id,
            ok: true,
            state,
            result,
        })),
        Err(error) => Response::Failure(FailureResponse {
            id,
            ok: false,
            error: error.into(),
        }),
    }
}

fn execute(request: Request) -> Result<(Project, Value), CoreError> {
    if request.method == "project.create" || request.method == "project.import" {
        if request.principal != crate::protocol::Principal::Human {
            return Err(CoreError::PermissionDenied);
        }
        if request.state.is_some() {
            return Err(CoreError::UnexpectedState);
        }
    }
    if request.method == "project.create" {
        let params: CreateParams = parse(request.params)?;
        let project = advance(
            create_project(params.name.as_deref(), params.fixture.unwrap_or(true)),
            "human",
            "Created project",
        )?;
        return Ok((project.clone(), json!({ "project": project })));
    }
    if request.method == "project.import" {
        let params: ImportParams = parse(request.params)?;
        validate_project(&params.project)?;
        let mut imported = params.project;
        imported.id = crate::ids::ProjectId::new_random();
        let project = advance(imported, "human", "Imported project")?;
        return Ok((project.clone(), json!({ "project": project })));
    }
    let project = request.state.ok_or(CoreError::MissingState)?;
    match request.method.as_str() {
        "project.get" => Ok((project.clone(), json!({ "project": project }))),
        "project.history" => {
            let hash = canonical_hash(&project)?;
            Ok((
                project.clone(),
                json!({ "revision": project.revision, "hash": hash, "history": project.history }),
            ))
        }
        "project.hash" => {
            let hash = canonical_hash(&project)?;
            Ok((
                project.clone(),
                json!({ "revision": project.revision, "hash": hash }),
            ))
        }
        "project.validate" => {
            validate_project(&project)?;
            Ok((project, json!({ "valid": true, "diagnostics": [] })))
        }
        "project.snapshot" => {
            let params: SnapshotParams = parse(request.params)?;
            let rendered = snapshot(&project, params.proposal_id.as_ref())?;
            Ok((project, json!({ "snapshot": rendered })))
        }
        "project.explain" => explain(project, parse(request.params)?),
        "change.dryRun" => dry_run(project, parse(request.params)?),
        "change.apply" => {
            let params: ChangeParams = parse(request.params)?;
            let changed = apply_change(
                &project,
                request.principal,
                ChangeRequest {
                    expected_revision: params.expected_revision,
                    command: params.command,
                },
            )?;
            Ok((changed.clone(), json!({ "revision": changed.revision })))
        }
        "proposal.create" => {
            let proposal = create_proposal(&project, parse(request.params)?)?;
            let mut changed = project;
            changed.proposals.push(proposal.clone());
            let author = match request.principal {
                crate::protocol::Principal::Human => "human",
                crate::protocol::Principal::Agent => "agent",
            };
            let changed = advance(changed, author, "Created proposal")?;
            Ok((changed, json!({ "proposal": proposal })))
        }
        "proposal.accept" => {
            let params: AcceptParams = parse(request.params)?;
            let outcome = accept_proposal(
                &project,
                request.principal,
                Acceptance {
                    expected_revision: params.expected_revision,
                    proposal_id: params.proposal_id.clone(),
                    hunk_ids: params.hunk_ids,
                },
            )?;
            let result = json!({
                "proposalId": params.proposal_id,
                "revision": outcome.project.revision,
                "accepted": outcome.accepted,
                "hunks": outcome.hunks,
                "diagnostics": outcome.diagnostics,
            });
            Ok((outcome.project, result))
        }
        "proposal.reject" => {
            let params: RejectParams = parse(request.params)?;
            let changed = reject_proposal(
                &project,
                request.principal,
                Rejection {
                    expected_revision: params.expected_revision,
                    proposal_id: params.proposal_id.clone(),
                },
            )?;
            Ok((changed, json!({ "proposalId": params.proposal_id })))
        }
        "project.undo" | "project.redo" => restore(
            project,
            request.principal,
            parse(request.params)?,
            &request.method,
        ),
        method => Err(CoreError::UnknownMethod(method.to_owned())),
    }
}

fn dry_run(project: Project, params: ChangeParams) -> Result<(Project, Value), CoreError> {
    let affected = affected_instances(&project, &params.command);
    let command_value = serde_json::to_value(&params.command)
        .map_err(|error| CoreError::Serialization(error.to_string()))?;
    let result = match preview_change(&project, &params.expected_revision, params.command) {
        Ok(_) => {
            json!({ "executable": true, "affectedInstances": affected, "hunks": [{ "command": command_value }], "diagnostics": [] })
        }
        Err(error) => {
            let diagnostic = Diagnostic {
                code: error.code().to_owned(),
                severity: Severity::Blocking,
                message: error.to_string(),
                entity_id: None,
            };
            json!({ "executable": false, "affectedInstances": affected, "hunks": [], "diagnostics": [diagnostic] })
        }
    };
    Ok((project, result))
}

fn explain(project: Project, params: ExplainParams) -> Result<(Project, Value), CoreError> {
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
    let modifiers: Vec<_> = instance
        .modifiers
        .iter()
        .filter_map(|id| project.modifiers.iter().find(|modifier| modifier.id == *id))
        .collect();
    let effective_notes: Vec<_> = snapshot(&project, None)?
        .notes
        .into_iter()
        .filter(|note| {
            note.instance_id == instance.id && note.start < instance.start + definition.length
        })
        .map(|note| crate::music::Note {
            id: note.origin.note_id,
            start: crate::time::Tick::from_ticks(note.start.ticks() - instance.start.ticks()),
            duration: note.duration,
            pitch: note.pitch,
            velocity: note.velocity,
        })
        .collect();
    let result = json!({ "revision": project.revision, "instanceId": instance.id, "instance": instance, "definition": definition, "effectiveNotes": effective_notes, "modifiers": modifiers, "overrides": instance.overrides });
    Ok((project, result))
}

fn restore(
    current: Project,
    principal: crate::protocol::Principal,
    params: RestoreParams,
    method: &str,
) -> Result<(Project, Value), CoreError> {
    if principal != crate::protocol::Principal::Human {
        return Err(CoreError::PermissionDenied);
    }
    require_revision(&current, &params.expected_revision)?;
    if current.id != params.snapshot.id {
        return Err(CoreError::InvalidProject(
            "restored snapshot belongs to a different project".to_owned(),
        ));
    }
    validate_project(&params.snapshot)?;
    let mut restored = params.snapshot;
    restored.revision = current.revision.clone();
    restored.history = current.history;
    let changed = advance(restored, "human", method)?;
    Ok((changed.clone(), json!({ "revision": changed.revision })))
}

fn parse<T: DeserializeOwned>(value: Value) -> Result<T, CoreError> {
    serde_json::from_value(value).map_err(|error| CoreError::InvalidParams(error.to_string()))
}
