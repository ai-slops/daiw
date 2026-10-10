use std::{
    fs,
    io::{Read, Write},
    thread,
    time::Duration,
};

use daiw_audio::{
    AudioError, NativePlayer, NativeWavPlayer, RenderInput, RenderRequest, WavStreamError,
    WavStreamState, render,
};

#[path = "daiw-play/arguments.rs"]
mod arguments;
#[path = "daiw-play/wav_input.rs"]
mod wav_input;
use arguments::{Invocation, StreamArguments};

const USAGE: &str = "Usage:\n  daiw-play snapshot.json\n  daiw-play - < snapshot.json\n  daiw-play --stream-wav ABS --sha256 HEX [--start-frame N] [--loop-start N --loop-end N] [--validate-only]\n\nStreaming requires canonical PCM16 stereo 48000 Hz WAV. A loop plays until the process is interrupted (Ctrl-C).\n";

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

fn run_legacy(path: Option<&str>) -> Result<(), AudioError> {
    let input = load_input(path)?;
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

fn run_stream(arguments: StreamArguments) -> Result<(), AudioError> {
    let (file, descriptor) = wav_input::validate(&arguments.path, &arguments.sha256)?;
    let frames = u32::try_from(descriptor.frames).map_err(WavStreamError::from)?;
    let options = arguments.options(frames)?;
    if arguments.validate_only {
        serde_json::to_writer(std::io::stdout().lock(), &descriptor)?;
        std::io::stdout().write_all(b"\n")?;
        return Ok(());
    }
    let mut player = NativeWavPlayer::new(file, options)?;
    player.play()?;
    loop {
        match player.state() {
            WavStreamState::Paused | WavStreamState::Playing => {
                if let Some(error) = player.take_error() {
                    return Err(error.into());
                }
                thread::sleep(Duration::from_millis(10));
            }
            WavStreamState::Ended | WavStreamState::Shutdown => {
                return player.shutdown().map_err(Into::into);
            }
            WavStreamState::Faulted => {
                return Err(player
                    .take_error()
                    .unwrap_or(WavStreamError::WorkerPanicked)
                    .into());
            }
        }
    }
}

fn run() -> Result<(), AudioError> {
    match arguments::parse(std::env::args().skip(1))? {
        Invocation::Help => {
            std::io::stdout().write_all(USAGE.as_bytes())?;
            Ok(())
        }
        Invocation::Legacy(path) => run_legacy(path.as_deref()),
        Invocation::Stream(arguments) => run_stream(arguments),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("daiw-play: {error}");
        std::process::exit(1);
    }
}
