use std::{
    fs,
    io::{Read, Write},
    thread,
    time::Duration,
};

use daiw_audio::{
    AudioError, NativePlayer, NativeWavPlayer, RenderInput, RenderRequest, WavStreamError,
    WavStreamState, list_native_output_devices, render,
};

#[path = "daiw-play/arguments.rs"]
mod arguments;
#[path = "daiw-play/control_io.rs"]
mod control_io;
#[path = "daiw-play/control_protocol.rs"]
mod control_protocol;
#[path = "daiw-play/control_session.rs"]
mod control_session;
#[path = "daiw-play/control_worker.rs"]
mod control_worker;
#[path = "daiw-play/wav_input.rs"]
mod wav_input;
use arguments::{ControlArguments, Invocation, StreamArguments};
use control_protocol::{ErrorBody, Output};
use control_session::{HeadlessSession, NativeSession};

const USAGE: &str = "Usage:\n  daiw-play snapshot.json\n  daiw-play - < snapshot.json\n  daiw-play --stream-wav ABS --sha256 HEX [--start-frame N] [--loop-start N --loop-end N] [--validate-only]\n  daiw-play --list-output-devices --json\n  daiw-play --control-wav ABS --sha256 HEX [--device-index N --device-sha256 HEX]\n  daiw-play --control-wav ABS --sha256 HEX --headless-callback-driver\n\nStreaming requires canonical PCM16 stereo 48000 Hz WAV. A loop plays until the process is interrupted (Ctrl-C).\n";

struct StartupFailure {
    code: &'static str,
    message: &'static str,
}

fn classify_startup_error(error: &AudioError) -> StartupFailure {
    match error {
        AudioError::Native(_) => StartupFailure {
            code: "AudioBackend",
            message: "audio backend could not be started",
        },
        AudioError::DeviceUnavailable | AudioError::NoOutputDevice => StartupFailure {
            code: "DeviceUnavailable",
            message: "audio output device is unavailable",
        },
        AudioError::Io(_)
        | AudioError::IntegerConversion(_)
        | AudioError::SliceConversion(_)
        | AudioError::WavStream(_) => StartupFailure {
            code: "InvalidWav",
            message: "WAV input could not be opened or validated",
        },
        _ => StartupFailure {
            code: "StartupFailed",
            message: "audio control could not be started",
        },
    }
}

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

fn run_control(arguments: ControlArguments) -> Result<(), AudioError> {
    let startup = (|| {
        let (file, descriptor) = wav_input::validate(&arguments.path, &arguments.sha256)?;
        let frames = u32::try_from(descriptor.frames).map_err(WavStreamError::from)?;
        let session: Box<dyn control_session::ControlSession> =
            if arguments.headless_callback_driver {
                Box::new(HeadlessSession::new(file)?)
            } else {
                let player = match arguments.device.as_ref() {
                    Some(selector) => NativeWavPlayer::new_with_device(
                        file,
                        daiw_audio::WavStreamOptions::default(),
                        selector,
                    )?,
                    None => NativeWavPlayer::new(file, daiw_audio::WavStreamOptions::default())?,
                };
                Box::new(NativeSession(player))
            };
        Ok::<_, AudioError>((session, frames))
    })();
    let (session, frames) = match startup {
        Ok(startup) => startup,
        Err(error) => {
            let failure = classify_startup_error(&error);
            control_io::write_output(
                &mut std::io::stdout().lock(),
                &Output::Fatal {
                    v: 1,
                    error: ErrorBody {
                        code: failure.code,
                        message: failure.message.into(),
                    },
                },
            )?;
            return Err(AudioError::ControlProtocol);
        }
    };
    control_worker::run(session, frames)
}

fn run() -> Result<(), AudioError> {
    match arguments::parse(std::env::args().skip(1))? {
        Invocation::Help => {
            std::io::stdout().write_all(USAGE.as_bytes())?;
            Ok(())
        }
        Invocation::Legacy(path) => run_legacy(path.as_deref()),
        Invocation::ListOutputDevices => {
            serde_json::to_writer(
                std::io::stdout().lock(),
                &serde_json::json!({"v": 1, "devices": list_native_output_devices()?}),
            )?;
            std::io::stdout().write_all(b"\n")?;
            Ok(())
        }
        Invocation::Stream(arguments) => run_stream(arguments),
        Invocation::Control(arguments) => run_control(arguments),
    }
}

fn main() {
    let control_mode = std::env::args().nth(1).as_deref() == Some("--control-wav");
    if let Err(error) = run() {
        if !control_mode {
            eprintln!("daiw-play: {error}");
        }
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::classify_startup_error;
    use daiw_audio::{AudioError, WavStreamError};

    #[test]
    fn native_startup_error_is_classified_as_audio_backend_without_detail() {
        // Given
        let error = AudioError::Native("private backend detail".into());

        // When
        let failure = classify_startup_error(&error);

        // Then
        assert_eq!(failure.code, "AudioBackend");
        assert_eq!(failure.message, "audio backend could not be started");
        assert!(!failure.message.contains("private"));
    }

    #[test]
    fn device_startup_errors_share_stable_device_unavailable_classification() {
        // Given
        let errors = [AudioError::NoOutputDevice, AudioError::DeviceUnavailable];

        for error in errors {
            // When
            let failure = classify_startup_error(&error);

            // Then
            assert_eq!(failure.code, "DeviceUnavailable");
            assert_eq!(failure.message, "audio output device is unavailable");
        }
    }

    #[test]
    fn wav_io_startup_error_is_invalid_wav_without_path() {
        // Given
        let error = AudioError::WavStream(WavStreamError::Io(std::io::Error::other(
            "/private/project.wav",
        )));

        // When
        let failure = classify_startup_error(&error);

        // Then
        assert_eq!(failure.code, "InvalidWav");
        assert_eq!(
            failure.message,
            "WAV input could not be opened or validated"
        );
        assert!(!failure.message.contains("private"));
    }

    #[test]
    fn wav_integrity_startup_error_is_invalid_wav() {
        // Given
        let error = AudioError::WavStream(WavStreamError::DescriptorMismatch);

        // When
        let failure = classify_startup_error(&error);

        // Then
        assert_eq!(failure.code, "InvalidWav");
        assert_eq!(
            failure.message,
            "WAV input could not be opened or validated"
        );
    }
}
