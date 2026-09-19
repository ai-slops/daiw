use std::{
    fs,
    io::{Read, Write},
    thread,
    time::Duration,
};

use daiw_audio::{AudioError, NativePlayer, RenderInput, RenderRequest, render};

const USAGE: &str = "Usage:\n  daiw-play snapshot.json\n  daiw-play - < snapshot.json\n";

fn load_input(path: Option<&str>) -> Result<Vec<u8>, AudioError> {
    match path {
        Some("-") | None => {
            let mut bytes = Vec::new();
            std::io::stdin().read_to_end(&mut bytes)?;
            Ok(bytes)
        }
        Some(path) => Ok(fs::read(path)?),
    }
}

fn run() -> Result<(), AudioError> {
    let mut arguments = std::env::args().skip(1);
    let path = arguments.next();
    if matches!(path.as_deref(), Some("--help" | "help")) {
        if let Some(extra) = arguments.next() {
            return Err(AudioError::InvalidArguments(format!(
                "unexpected argument after help: {extra}"
            )));
        }
        std::io::stdout().write_all(USAGE.as_bytes())?;
        return Ok(());
    }
    if let Some(extra) = arguments.next() {
        return Err(AudioError::InvalidArguments(format!(
            "unexpected argument after snapshot path: {extra}"
        )));
    }
    let input = load_input(path.as_deref())?;
    let wire: RenderInput = serde_json::from_slice(&input)?;
    let rendered = render(&RenderRequest::from(wire))?;
    let duration = Duration::from_secs_f64(
        rendered.samples.len() as f64
            / f64::from(rendered.channels)
            / f64::from(rendered.sample_rate),
    );
    let player = NativePlayer::new(rendered)?;
    player.play();
    thread::sleep(duration);
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("daiw-play: {error}");
        std::process::exit(1);
    }
}
