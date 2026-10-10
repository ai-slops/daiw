use std::io::{Read, Write};
use std::path::Path;

use daiw_audio::{
    AudioError, FileRenderInput, RenderInput, RenderRequest, decode_file, decode_to_pcm, render,
    render_file_backed, render_file_backed_to_wav, wav,
};

const USAGE: &str = "Usage:\n  daiw-render < snapshot.json > mix.wav\n  daiw-render --decode /absolute/path/to/audio\n  daiw-render --decode-file --input ABS --output ABS_NEW_TEMP\n  daiw-render --render-file-backed --source-root ABS < request.json > mix.wav\n  daiw-render --render-to-file --source-root ABS --output ABS_NEW_TEMP < request.json\n";

struct DescriptorPublication<'a> {
    output: &'a Path,
    published: bool,
}

impl Drop for DescriptorPublication<'_> {
    fn drop(&mut self) {
        if !self.published {
            let _ = std::fs::remove_file(self.output);
        }
    }
}

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

fn render_to_file_stdin(root: &Path, output: &Path) -> Result<(), AudioError> {
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input)?;
    let wire: FileRenderInput = serde_json::from_slice(&input)?;
    let descriptor = render_file_backed_to_wav(wire, root, output)?;
    let mut publication = DescriptorPublication {
        output,
        published: false,
    };
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, &descriptor)
        .map_err(|error| AudioError::Io(std::io::Error::other(error)))?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    publication.published = true;
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
        Some("--render-to-file") => {
            if arguments.next().as_deref() != Some("--source-root") {
                return Err(AudioError::InvalidArguments(
                    "--render-to-file requires --source-root ABS --output ABS_NEW_TEMP".into(),
                ));
            }
            let root = arguments
                .next()
                .ok_or_else(|| AudioError::InvalidArguments("missing --source-root path".into()))?;
            if arguments.next().as_deref() != Some("--output") {
                return Err(AudioError::InvalidArguments(
                    "--render-to-file requires --output ABS_NEW_TEMP".into(),
                ));
            }
            let output = arguments
                .next()
                .ok_or_else(|| AudioError::InvalidArguments("missing --output path".into()))?;
            if let Some(extra) = arguments.next() {
                return Err(AudioError::InvalidArguments(format!(
                    "unexpected argument after output path: {extra}"
                )));
            }
            render_to_file_stdin(Path::new(&root), Path::new(&output))
        }
        Some(argument) => Err(AudioError::InvalidArguments(format!(
            "unknown argument: {argument}"
        ))),
        None => render_stdin(),
    }
}

fn main() {
    let first_mode = std::env::args().nth(1);
    if let Err(error) = run() {
        eprintln!("daiw-render: {error}");
        std::process::exit(failure_exit_code(first_mode.as_deref(), &error));
    }
}

fn failure_exit_code(mode: Option<&str>, error: &AudioError) -> i32 {
    let new_file_mode = matches!(
        mode,
        Some("--decode-file" | "--render-file-backed" | "--render-to-file")
    );
    let resource_error = matches!(
        error,
        AudioError::DecodedPacketTooLarge
            | AudioError::PcmDurationTooLong
            | AudioError::PcmPayloadTooLarge
            | AudioError::TooManySources { .. }
            | AudioError::SourceFilesTooLarge
            | AudioError::RenderTooLarge { .. }
            | AudioError::TooManyActiveTracks { .. }
    );
    let infrastructure_error = matches!(
        error,
        AudioError::Io(_)
            | AudioError::InvalidSourceRoot
            | AudioError::InvalidRenderOutputPath
            | AudioError::InvalidSignalGraph { .. }
            | AudioError::SignalGraphLimitExceeded { .. }
    );
    if mode == Some("--render-to-file") && infrastructure_error {
        3
    } else if new_file_mode && resource_error {
        2
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::failure_exit_code;
    use daiw_audio::AudioError;

    #[test]
    fn render_to_file_classifies_internal_graph_failures_as_infrastructure() {
        // Given
        let invalid = AudioError::InvalidSignalGraph { issue: "test" };
        let limit = AudioError::SignalGraphLimitExceeded {
            resource: "nodes",
            limit: 1,
            actual: 2,
        };

        // When / Then
        assert_eq!(failure_exit_code(Some("--render-to-file"), &invalid), 3);
        assert_eq!(failure_exit_code(Some("--render-to-file"), &limit), 3);
    }

    #[test]
    fn render_to_file_keeps_active_track_limit_as_resource_failure() {
        // Given
        let error = AudioError::TooManyActiveTracks {
            limit: 256,
            actual: 257,
        };

        // When / Then
        assert_eq!(failure_exit_code(Some("--render-to-file"), &error), 2);
    }
}
