use crate::{
    Command, CoreError, Principal, Project, Request, Response, Revision, canonical_hash, dispatch,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::{self, Read, Write};

const HELP: &str = "daiw-core [COMMAND]\n\nCommands:\n  fixture [NAME]       Print the shared-chorus fixture as legacy Project JSON\n  export               Read project/native JSON and emit a versioned envelope\n  validate             Validate project/native JSON from stdin\n  explain INSTANCE_ID  Read project/native JSON and explain an instance\n  dry-run              Read {state,expectedRevision,command} JSON and preview it\n  help, --help         Show this help\n\nWith no command, daiw-core runs the JSON-lines worker.\n";

pub fn run(args: &[String]) -> Result<bool, CoreError> {
    let Some(command) = args.first().map(String::as_str) else {
        return Ok(false);
    };
    match command {
        "help" | "--help" | "-h" => write_text(HELP)?,
        "fixture" => fixture(args)?,
        "export" => write_json(&crate::native_format::export(&read_project()?)?)?,
        "validate" => validate()?,
        "explain" => explain(args)?,
        "dry-run" => dry_run()?,
        other => {
            return Err(CoreError::InvalidParams(format!(
                "unknown CLI command: {other}"
            )));
        }
    }
    Ok(true)
}

fn fixture(args: &[String]) -> Result<(), CoreError> {
    let name = if args.len() > 1 {
        args[1..].join(" ")
    } else {
        "Shared Chorus Study".to_owned()
    };
    let response = dispatch(Request {
        id: "cli-fixture".to_owned(),
        principal: Principal::Human,
        state: None,
        method: "project.create".to_owned(),
        params: json!({ "name": name, "fixture": true }),
    });
    match response {
        Response::Success(response) => write_json(&response.state),
        Response::Failure(response) => Err(CoreError::InvalidProject(response.error.message)),
    }
}

fn validate() -> Result<(), CoreError> {
    let project = read_project()?;
    write_json(&json!({ "valid": true, "hash": canonical_hash(&project)? }))
}

fn explain(args: &[String]) -> Result<(), CoreError> {
    let instance_id = args
        .get(1)
        .ok_or_else(|| CoreError::InvalidParams("explain requires INSTANCE_ID".to_owned()))?;
    let project = read_project()?;
    let response = dispatch(Request {
        id: "cli-explain".to_owned(),
        principal: Principal::Human,
        state: Some(project),
        method: "project.explain".to_owned(),
        params: json!({ "instanceId": instance_id }),
    });
    match response {
        Response::Success(response) => write_json(&response.result),
        Response::Failure(response) => Err(CoreError::InvalidParams(response.error.message)),
    }
}

fn dry_run() -> Result<(), CoreError> {
    let input: DryRunInput = read_json()?;
    let response = dispatch(Request {
        id: "cli-dry-run".to_owned(),
        principal: Principal::Agent,
        state: Some(crate::native_format::import(input.state)?.0),
        method: "change.dryRun".to_owned(),
        params: json!({ "expectedRevision": input.expected_revision, "command": input.command }),
    });
    match response {
        Response::Success(response) => write_json(&response.result),
        Response::Failure(response) => Err(CoreError::InvalidParams(response.error.message)),
    }
}

fn read_json<T: for<'de> Deserialize<'de>>() -> Result<T, CoreError> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|error| CoreError::Io(error.to_string()))?;
    serde_json::from_str(&input).map_err(|error| CoreError::InvalidParams(error.to_string()))
}

fn read_project() -> Result<Project, CoreError> {
    Ok(crate::native_format::import(read_json()?)?.0)
}

fn write_json<T: Serialize>(value: &T) -> Result<(), CoreError> {
    let mut output = io::BufWriter::new(io::stdout().lock());
    serde_json::to_writer_pretty(&mut output, value)
        .map_err(|error| CoreError::Serialization(error.to_string()))?;
    output
        .write_all(b"\n")
        .map_err(|error| CoreError::Io(error.to_string()))
}

fn write_text(value: &str) -> Result<(), CoreError> {
    io::stdout()
        .lock()
        .write_all(value.as_bytes())
        .map_err(|error| CoreError::Io(error.to_string()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DryRunInput {
    state: serde_json::Value,
    expected_revision: Revision,
    command: Command,
}
