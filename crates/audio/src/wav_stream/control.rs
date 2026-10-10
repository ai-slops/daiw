use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread::{JoinHandle, Thread};

use super::shared::{Shared, lock, unpack};
use super::{FramePosition, WavStreamError, WavStreamState};

pub struct WavStreamControl {
    pub(crate) shared: Arc<Shared>,
    pub(crate) worker_thread: Thread,
    pub(crate) worker: Option<JoinHandle<()>>,
    pub(crate) frames: u32,
}

impl WavStreamControl {
    pub(crate) fn new(
        shared: Arc<Shared>,
        worker_thread: Thread,
        worker: JoinHandle<()>,
        frames: u32,
    ) -> Self {
        Self {
            shared,
            worker_thread,
            worker: Some(worker),
            frames,
        }
    }

    pub fn play(&self) -> Result<(), WavStreamError> {
        let _plan = lock(&self.shared.plan);
        loop {
            match self.operational_state()? {
                WavStreamState::Paused => {
                    if self.shared.transition_state(
                        self.shared.state_snapshot().0,
                        WavStreamState::Paused,
                        WavStreamState::Playing,
                    ) {
                        return Ok(());
                    }
                }
                WavStreamState::Playing | WavStreamState::Ended => return Ok(()),
                WavStreamState::Faulted => return Err(WavStreamError::StreamFaulted),
                WavStreamState::Shutdown => return Err(WavStreamError::StreamShutdown),
            }
        }
    }

    pub fn pause(&self) -> Result<(), WavStreamError> {
        self.pause_inner(|| {})
    }

    #[cfg(test)]
    pub(crate) fn pause_with_hook(&self, hook: impl FnOnce()) -> Result<(), WavStreamError> {
        self.pause_inner(hook)
    }

    #[must_use]
    pub fn position(&self) -> FramePosition {
        unpack(self.shared.cursor.load(Ordering::Acquire)).1
    }

    #[must_use]
    pub fn state(&self) -> WavStreamState {
        self.shared.state()
    }

    pub fn shutdown(&mut self) -> Result<(), WavStreamError> {
        {
            let plan = lock(&self.shared.plan);
            self.shared.shutdown.store(true, Ordering::Release);
            self.shared
                .publish_state(plan.generation, WavStreamState::Shutdown);
        }
        self.worker_thread.unpark();
        if let Some(worker) = self.worker.take() {
            worker.join().map_err(|_| WavStreamError::WorkerPanicked)?;
        }
        Ok(())
    }

    pub(crate) fn operational_state(&self) -> Result<WavStreamState, WavStreamError> {
        match self.shared.state() {
            WavStreamState::Faulted => Err(WavStreamError::StreamFaulted),
            WavStreamState::Shutdown => Err(WavStreamError::StreamShutdown),
            state @ (WavStreamState::Paused | WavStreamState::Playing | WavStreamState::Ended) => {
                Ok(state)
            }
        }
    }

    fn pause_inner(&self, hook: impl FnOnce()) -> Result<(), WavStreamError> {
        let plan = lock(&self.shared.plan);
        let mut hook = Some(hook);
        loop {
            match self.operational_state()? {
                WavStreamState::Playing => {
                    if let Some(run) = hook.take() {
                        run();
                    }
                    if self.shared.transition_state(
                        plan.generation,
                        WavStreamState::Playing,
                        WavStreamState::Paused,
                    ) {
                        return Ok(());
                    }
                }
                WavStreamState::Paused | WavStreamState::Ended => return Ok(()),
                WavStreamState::Faulted => return Err(WavStreamError::StreamFaulted),
                WavStreamState::Shutdown => return Err(WavStreamError::StreamShutdown),
            }
        }
    }
}

impl Drop for WavStreamControl {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
