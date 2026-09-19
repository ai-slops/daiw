use std::io::{Read, Write};
use std::path::Path;

use daiw_audio::{AudioError, RenderInput, RenderRequest, decode_file, render, wav};

const USAGE: &str = "Usage:\n  daiw-render < snapshot.json > mix.wav\n  daiw-render --decode /absolute/path/to/audio\n";

fn render_stdin() -> Result<(), AudioError> {
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input)?;
    let wire: RenderInput = serde_json::from_slice(&input)?;
    let rendered = render(&RenderRequest::from(wire))?;
    std::io::stdout().write_all(&wav(&rendered)?)?;
    Ok(())
}

fn run() -> Result<(), AudioError> {
    let mut arguments = std::env::args().skip(1);
    match arguments.next().as_deref() {
        Some("--help" | "help") => {
            if let Some(extra) = arguments.next() {
                return Err(AudioError::InvalidArguments(format!(
                    "unexpected argument after help: {extra}"
                )));
            }
            std::io::stdout().write_all(USAGE.as_bytes())?;
            Ok(())
        }
        Some("--decode") => {
            let path = arguments.next().ok_or(AudioError::InvalidDecodePath)?;
            if let Some(extra) = arguments.next() {
                return Err(AudioError::InvalidArguments(format!(
                    "unexpected argument after decode path: {extra}"
                )));
            }
            let decoded = decode_file(Path::new(&path))?;
            serde_json::to_writer(std::io::stdout(), &decoded)?;
            Ok(())
        }
        Some(argument) => Err(AudioError::InvalidArguments(format!(
            "unknown argument: {argument}"
        ))),
        None => render_stdin(),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("daiw-render: {error}");
        std::process::exit(1);
    }
}
