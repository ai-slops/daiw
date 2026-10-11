use std::path::PathBuf;

use daiw_audio::{
    AudioError, FrameLoopRange, FramePosition, Sha256Digest, WavStreamError, WavStreamOptions,
};

pub enum Invocation {
    Help,
    Legacy(Option<String>),
    ListOutputDevices,
    Stream(StreamArguments),
    Control(ControlArguments),
}

pub struct ControlArguments {
    pub path: PathBuf,
    pub sha256: String,
    pub device: Option<daiw_audio::NativeDeviceSelector>,
    pub headless_callback_driver: bool,
}

pub struct StreamArguments {
    pub path: PathBuf,
    pub sha256: String,
    pub start_frame: FramePosition,
    pub loop_bounds: Option<(FramePosition, FramePosition)>,
    pub validate_only: bool,
}

impl StreamArguments {
    pub fn options(&self, frames: u32) -> Result<WavStreamOptions, WavStreamError> {
        if self.start_frame.get() > frames {
            return Err(WavStreamError::InvalidStartFrame);
        }
        let loop_range = match self.loop_bounds {
            Some((start, end)) => Some(FrameLoopRange::new(start, end, frames)?),
            None => None,
        };
        Ok(WavStreamOptions {
            start_frame: self.start_frame,
            loop_range,
        })
    }
}

pub fn parse(arguments: impl IntoIterator<Item = String>) -> Result<Invocation, AudioError> {
    let mut arguments = arguments.into_iter();
    let Some(first) = arguments.next() else {
        return Ok(Invocation::Legacy(None));
    };
    match first.as_str() {
        "--help" | "help" => {
            reject_extra(arguments, "help")?;
            Ok(Invocation::Help)
        }
        "--stream-wav" => parse_stream(arguments).map(Invocation::Stream),
        "--control-wav" => parse_control(arguments).map(Invocation::Control),
        "--list-output-devices" => {
            if arguments.next().as_deref() != Some("--json") {
                return Err(invalid("--list-output-devices requires --json"));
            }
            reject_extra(arguments, "--json")?;
            Ok(Invocation::ListOutputDevices)
        }
        value if value.starts_with('-') && value != "-" => {
            Err(invalid(format!("unknown argument: {value}")))
        }
        path => {
            reject_extra(arguments, "snapshot path")?;
            Ok(Invocation::Legacy(Some(path.to_owned())))
        }
    }
}

fn parse_control(
    mut arguments: impl Iterator<Item = String>,
) -> Result<ControlArguments, AudioError> {
    let path = PathBuf::from(take_value(&mut arguments, "--control-wav")?);
    if path.as_os_str().len() > 4096 {
        return Err(invalid("--control-wav path exceeds 4096 bytes"));
    }
    let mut sha256 = None;
    let mut device_index = None;
    let mut device_sha256 = None;
    let mut headless_callback_driver = false;
    while let Some(flag) = arguments.next() {
        match flag.as_str() {
            "--sha256" => {
                reject_duplicate(&sha256, "--sha256")?;
                let value = take_value(&mut arguments, "--sha256")?;
                Sha256Digest::parse(&value)?;
                sha256 = Some(value);
            }
            "--device-index" => {
                reject_duplicate(&device_index, "--device-index")?;
                let value = take_value(&mut arguments, "--device-index")?;
                device_index = Some(value.parse::<usize>().map_err(|_| {
                    invalid(format!("device index must be an unsigned integer: {value}"))
                })?);
            }
            "--device-sha256" => {
                reject_duplicate(&device_sha256, "--device-sha256")?;
                let value = take_value(&mut arguments, "--device-sha256")?;
                Sha256Digest::parse(&value)?;
                device_sha256 = Some(value);
            }
            "--headless-callback-driver" if !headless_callback_driver => {
                headless_callback_driver = true;
            }
            "--headless-callback-driver" => {
                return Err(invalid("duplicate --headless-callback-driver"));
            }
            _ => return Err(invalid(format!("unknown argument: {flag}"))),
        }
    }
    let sha256 = sha256.ok_or_else(|| invalid("missing required --sha256"))?;
    let device = match (device_index, device_sha256) {
        (Some(index), Some(digest)) => Some(daiw_audio::NativeDeviceSelector::new(index, &digest)?),
        (None, None) => None,
        (Some(_), None) | (None, Some(_)) => {
            return Err(invalid(
                "--device-index and --device-sha256 must be used together",
            ));
        }
    };
    if headless_callback_driver && device.is_some() {
        return Err(invalid(
            "--headless-callback-driver cannot select a physical device",
        ));
    }
    Ok(ControlArguments {
        path,
        sha256,
        device,
        headless_callback_driver,
    })
}

fn parse_stream(
    mut arguments: impl Iterator<Item = String>,
) -> Result<StreamArguments, AudioError> {
    let path = PathBuf::from(take_value(&mut arguments, "--stream-wav")?);
    let mut sha256 = None;
    let mut start_frame = None;
    let mut loop_start = None;
    let mut loop_end = None;
    let mut validate_only = false;
    while let Some(flag) = arguments.next() {
        match flag.as_str() {
            "--sha256" => {
                reject_duplicate(&sha256, "--sha256")?;
                let value = take_value(&mut arguments, "--sha256")?;
                Sha256Digest::parse(&value)?;
                sha256 = Some(value);
            }
            "--start-frame" => {
                reject_duplicate(&start_frame, "--start-frame")?;
                start_frame = Some(parse_frame(take_value(&mut arguments, "--start-frame")?)?);
            }
            "--loop-start" => {
                reject_duplicate(&loop_start, "--loop-start")?;
                loop_start = Some(parse_frame(take_value(&mut arguments, "--loop-start")?)?);
            }
            "--loop-end" => {
                reject_duplicate(&loop_end, "--loop-end")?;
                loop_end = Some(parse_frame(take_value(&mut arguments, "--loop-end")?)?);
            }
            "--validate-only" if !validate_only => validate_only = true,
            "--validate-only" => return Err(invalid("duplicate --validate-only")),
            _ => return Err(invalid(format!("unknown argument: {flag}"))),
        }
    }
    let sha256 = sha256.ok_or_else(|| invalid("missing required --sha256"))?;
    let loop_bounds = match (loop_start, loop_end) {
        (Some(start), Some(end)) => Some((start, end)),
        (None, None) => None,
        (Some(_), None) | (None, Some(_)) => {
            return Err(invalid("--loop-start and --loop-end must be used together"));
        }
    };
    Ok(StreamArguments {
        path,
        sha256,
        start_frame: start_frame.unwrap_or_default(),
        loop_bounds,
        validate_only,
    })
}

fn take_value(
    arguments: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<String, AudioError> {
    let value = arguments
        .next()
        .ok_or_else(|| invalid(format!("missing value for {flag}")))?;
    if value.starts_with("--") {
        return Err(invalid(format!("missing value for {flag}")));
    }
    Ok(value)
}

fn parse_frame(value: String) -> Result<FramePosition, AudioError> {
    value
        .parse::<u32>()
        .map(FramePosition::new)
        .map_err(|_| invalid(format!("frame must be an unsigned 32-bit integer: {value}")))
}

fn reject_duplicate<T>(value: &Option<T>, flag: &str) -> Result<(), AudioError> {
    if value.is_some() {
        return Err(invalid(format!("duplicate {flag}")));
    }
    Ok(())
}

fn reject_extra(
    mut arguments: impl Iterator<Item = String>,
    after: &str,
) -> Result<(), AudioError> {
    if let Some(extra) = arguments.next() {
        return Err(invalid(format!(
            "unexpected argument after {after}: {extra}"
        )));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> AudioError {
    AudioError::InvalidArguments(message.into())
}
