use std::io::{BufRead, Read, Write};
use std::sync::mpsc::{Receiver, SyncSender};

use daiw_audio::AudioError;

use super::control_protocol::Output;

const MAX_LINE_BYTES: usize = 4096;
const COMMAND_CAPACITY: usize = 8;

pub enum Input {
    Line(Vec<u8>),
    TooLong,
    Io(std::io::Error),
    Eof,
}

pub fn input_receiver() -> Result<Receiver<Input>, std::io::Error> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(COMMAND_CAPACITY);
    let reader = std::thread::Builder::new()
        .name("daiw-play-control-input".to_owned())
        .spawn(move || read_input(std::io::stdin().lock(), &sender))?;
    drop(reader);
    Ok(receiver)
}

fn read_input(mut input: impl BufRead, sender: &SyncSender<Input>) -> Result<(), std::io::Error> {
    loop {
        let mut line = Vec::with_capacity(256);
        let count = match input
            .by_ref()
            .take(u64::try_from(MAX_LINE_BYTES + 1).map_err(std::io::Error::other)?)
            .read_until(b'\n', &mut line)
        {
            Ok(count) => count,
            Err(error) => {
                let _ = sender.send(Input::Io(error));
                return Ok(());
            }
        };
        if count == 0 {
            let _ = sender.send(Input::Eof);
            return Ok(());
        }
        if line.len() > MAX_LINE_BYTES {
            let _ = sender.send(Input::TooLong);
            return Ok(());
        }
        if sender.send(Input::Line(line)).is_err() {
            return Ok(());
        }
    }
}

pub fn write_output(output: &mut dyn Write, value: &Output<'_>) -> Result<(), AudioError> {
    let line = serde_json::to_vec(value)?;
    if line.len() + 1 > MAX_LINE_BYTES {
        return Err(AudioError::InvalidArguments(
            "control output exceeds 4096 bytes".into(),
        ));
    }
    output.write_all(&line)?;
    output.write_all(b"\n")?;
    output.flush()?;
    Ok(())
}
