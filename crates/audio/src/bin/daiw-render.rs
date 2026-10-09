use std::io::{Read, Write};
use std::path::Path;

use daiw_audio::{
    AudioError, FileRenderInput, RenderInput, RenderRequest, decode_file, decode_to_pcm, render,
    render_file_backed, wav,
};

const USAGE: &str = "Usage:\n  daiw-render < snapshot.json > mix.wav\n  daiw-render --decode /absolute/path/to/audio\n  daiw-render --decode-file --input ABS --output ABS_NEW_TEMP\n  daiw-render --render-file-backed --source-root ABS < request.json > mix.wav\n";

fn render_stdin() -> Result<(), AudioError> {
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input)?;
    let wire: RenderInput = serde_json::from_slice(&input)?;
    let rendered = render(&RenderRequest::from(wire))?;
    std::io::stdout().write_all(&wav(&rendered)?)?;
    Ok(())
}

fn render_file_stdin(root: &Path) -> Result<(), AudioError> {
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input)?;
    let wire: FileRenderInput = serde_json::from_slice(&input)?;
    let rendered = render_file_backed(wire, root)?;
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
        Some("--decode-file") => {
            if arguments.next().as_deref() != Some("--input") {
                return Err(AudioError::InvalidArguments(
                    "--decode-file requires --input ABS --output ABS_NEW_TEMP".into(),
                ));
            }
            let input = arguments.next().ok_or_else(|| {
                AudioError::InvalidArguments("missing --decode-file input path".into())
            })?;
            if arguments.next().as_deref() != Some("--output") {
                return Err(AudioError::InvalidArguments(
                    "--decode-file requires --output ABS_NEW_TEMP".into(),
                ));
            }
            let output = arguments.next().ok_or_else(|| {
                AudioError::InvalidArguments("missing --decode-file output path".into())
            })?;
            if let Some(extra) = arguments.next() {
                return Err(AudioError::InvalidArguments(format!(
                    "unexpected argument after output path: {extra}"
                )));
            }
            let descriptor = decode_to_pcm(Path::new(&input), Path::new(&output))?;
            serde_json::to_writer(std::io::stdout(), &descriptor)?;
            Ok(())
        }
        Some("--render-file-backed") => {
            if arguments.next().as_deref() != Some("--source-root") {
                return Err(AudioError::InvalidArguments(
                    "--render-file-backed requires --source-root ABS".into(),
                ));
            }
            let root = arguments
                .next()
                .ok_or_else(|| AudioError::InvalidArguments("missing --source-root path".into()))?;
            if let Some(extra) = arguments.next() {
                return Err(AudioError::InvalidArguments(format!(
                    "unexpected argument after source root: {extra}"
                )));
            }
            render_file_stdin(Path::new(&root))
        }
        Some(argument) => Err(AudioError::InvalidArguments(format!(
            "unknown argument: {argument}"
        ))),
        None => render_stdin(),
    }
}

fn main() {
    let new_file_mode = matches!(
        std::env::args().nth(1).as_deref(),
        Some("--decode-file" | "--render-file-backed")
    );
    if let Err(error) = run() {
        eprintln!("daiw-render: {error}");
        let resource_error = matches!(
            error,
            AudioError::DecodedPacketTooLarge
                | AudioError::PcmDurationTooLong
                | AudioError::PcmPayloadTooLarge
                | AudioError::TooManySources { .. }
                | AudioError::SourceFilesTooLarge
                | AudioError::RenderTooLarge { .. }
        );
        std::process::exit(if new_file_mode && resource_error {
            2
        } else {
            1
        });
    }
}
