use daiw_core::{CoreError, ErrorBody, FailureResponse, Request, Response, WorkerRuntime};
use std::io::{self, BufRead, Write};

fn main() {
    if let Err(error) = run() {
        eprintln!("daiw-core: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), CoreError> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if daiw_core::run_cli(&args)? {
        return Ok(());
    }
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    let mut runtime = WorkerRuntime::default();
    for line in stdin.lock().lines() {
        let line = line.map_err(|error| CoreError::Io(error.to_string()))?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Request>(&line) {
            Ok(request) => runtime.dispatch(request),
            Err(error) => Response::Failure(FailureResponse {
                id: String::new(),
                ok: false,
                error: ErrorBody {
                    code: "InvalidRequest".to_owned(),
                    message: error.to_string(),
                },
            }),
        };
        serde_json::to_writer(&mut stdout, &response)
            .map_err(|error| CoreError::Serialization(error.to_string()))?;
        stdout
            .write_all(b"\n")
            .map_err(|error| CoreError::Io(error.to_string()))?;
        stdout
            .flush()
            .map_err(|error| CoreError::Io(error.to_string()))?;
    }
    Ok(())
}
