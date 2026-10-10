mod callback;
#[cfg(test)]
mod callback_resume;
#[cfg(test)]
mod concurrency;
mod control;
#[cfg(test)]
mod control_concurrency;
mod control_update;
pub(crate) mod reporter;
mod shared;
mod types;
mod validate;
mod worker;

use std::fs::File;
use std::sync::Arc;
use std::thread::JoinHandle;

use rtrb::RingBuffer;

pub use callback::WavStreamCallback;
pub use control::WavStreamControl;
pub use types::{
    FrameLoopRange, FramePosition, MAX_WAV_STREAM_FRAMES, STREAM_BLOCK_FRAMES, STREAM_RING_BLOCKS,
    Sha256Digest, StreamGeneration, WavStreamError, WavStreamOptions, WavStreamState,
    WavStreamStats,
};
pub use validate::validate_wav_file;

use shared::Shared;
use worker::{StreamBlock, run_worker};

pub struct WavStreamFile {
    file: File,
    frames: u32,
}

impl WavStreamFile {
    pub fn open(
        self,
        options: WavStreamOptions,
    ) -> Result<(WavStreamControl, WavStreamCallback), WavStreamError> {
        streaming_pair(self, options)
    }

    #[must_use]
    pub const fn frames(&self) -> u32 {
        self.frames
    }
}

pub fn streaming_pair(
    file: WavStreamFile,
    options: WavStreamOptions,
) -> Result<(WavStreamControl, WavStreamCallback), WavStreamError> {
    validate_options(file.frames, options)?;
    let shared = Arc::new(Shared::new(options));
    let (producer, consumer) = RingBuffer::<StreamBlock>::new(STREAM_RING_BLOCKS);
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
    let worker_shared = Arc::clone(&shared);
    let frames = file.frames;
    let worker = std::thread::Builder::new()
        .name("daiw-wav-stream".to_owned())
        .stack_size(1024 * 1024)
        .spawn(move || run_worker(file.file, frames, producer, worker_shared, ready_tx))?;
    let worker_thread = worker.thread().clone();
    let worker = await_worker_ready(&shared, worker, ready_rx)?;
    if let Some(error) = lock_error(&shared) {
        let _ = worker.join();
        return Err(error);
    }
    Ok((
        WavStreamControl::new(Arc::clone(&shared), worker_thread, worker, frames),
        WavStreamCallback::new(consumer, shared),
    ))
}

fn await_worker_ready(
    shared: &Shared,
    worker: JoinHandle<()>,
    ready_rx: std::sync::mpsc::Receiver<()>,
) -> Result<JoinHandle<()>, WavStreamError> {
    if ready_rx.recv().is_ok() {
        return Ok(worker);
    }
    shared
        .shutdown
        .store(true, std::sync::atomic::Ordering::Release);
    worker.thread().unpark();
    let _ = worker.join();
    Err(WavStreamError::WorkerPanicked)
}

fn validate_options(frames: u32, options: WavStreamOptions) -> Result<(), WavStreamError> {
    if options.start_frame.get() > frames {
        return Err(WavStreamError::InvalidStartFrame);
    }
    if options
        .loop_range
        .is_some_and(|range| range.end().get() > frames)
    {
        return Err(WavStreamError::InvalidLoopRange);
    }
    Ok(())
}

fn lock_error(shared: &Shared) -> Option<WavStreamError> {
    shared::lock(&shared.error).take()
}

const _: () = assert!(!std::mem::needs_drop::<StreamBlock>());

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::{Shared, WavStreamError, WavStreamOptions, await_worker_ready};

    #[test]
    fn readiness_disconnect_wakes_and_joins_worker() {
        let shared = Shared::new(WavStreamOptions::default());
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
        let finished = Arc::new(AtomicBool::new(false));
        let worker_finished = Arc::clone(&finished);
        let worker = std::thread::spawn(move || {
            drop(ready_tx);
            std::thread::park();
            worker_finished.store(true, Ordering::Release);
        });

        let result = await_worker_ready(&shared, worker, ready_rx);

        assert!(matches!(result, Err(WavStreamError::WorkerPanicked)));
        assert!(shared.shutdown.load(Ordering::Acquire));
        assert!(finished.load(Ordering::Acquire));
    }
}
