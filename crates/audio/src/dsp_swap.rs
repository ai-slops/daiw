use std::mem;
use std::sync::atomic::{Ordering, fence};

use rtrb::{Consumer, Producer, PushError, RingBuffer};

use crate::{
    AudioError, CompiledDspProgram, DspExecutor, DspProcessError, DspProcessReport,
    DspProcessState, ResidentDspSources, Snapshot,
};

#[derive(Debug, thiserror::Error)]
pub enum DspPublicationError {
    #[error("DSP sample rate mismatch: expected {expected}, received {actual}")]
    SampleRateMismatch { expected: u32, actual: u32 },
    #[error("DSP callback is disconnected")]
    Disconnected,
    #[error("DSP publication queue is full")]
    QueueFull,
    #[error("DSP program build failed: {0}")]
    Build(#[source] AudioError),
}

pub struct DspSwap;

/// Control-thread endpoint for publishing and reclaiming prepared DSP executors.
pub struct DspSwapControl<'a> {
    sample_rate: u32,
    pending: Producer<DspExecutor<'a>>,
    retired: Consumer<DspExecutor<'a>>,
}

/// Real-time endpoint that activates publications only at process boundaries.
///
/// Ordinary endpoint destruction can drop executors and is not real-time safe. After the host
/// quiesces processing, move this value to the control thread and use [`DspSwapControl::shutdown`].
pub struct DspSwapCallback<'a> {
    current: DspExecutor<'a>,
    pending: Consumer<DspExecutor<'a>>,
    retired: Producer<DspExecutor<'a>>,
    deferred: Option<DspExecutor<'a>>,
}

impl DspSwap {
    #[must_use]
    pub fn channel<'a>(mut initial: DspExecutor<'a>) -> (DspSwapControl<'a>, DspSwapCallback<'a>) {
        initial.reset();
        let sample_rate = initial.program().sample_rate();
        let (pending_producer, pending_consumer) = RingBuffer::new(1);
        let (retired_producer, retired_consumer) = RingBuffer::new(1);
        (
            DspSwapControl {
                sample_rate,
                pending: pending_producer,
                retired: retired_consumer,
            },
            DspSwapCallback {
                current: initial,
                pending: pending_consumer,
                retired: retired_producer,
                deferred: None,
            },
        )
    }
}

impl<'a> DspSwapControl<'a> {
    /// Queues a reset executor for a later callback boundary.
    ///
    /// Success means queued, not activated. `Disconnected` is reported only when disconnection is
    /// observed before the push and does not guarantee concurrent endpoint liveness.
    pub fn enqueue(&mut self, mut executor: DspExecutor<'a>) -> Result<(), DspPublicationError> {
        let actual = executor.program().sample_rate();
        if actual != self.sample_rate {
            return Err(DspPublicationError::SampleRateMismatch {
                expected: self.sample_rate,
                actual,
            });
        }
        if self.pending.is_abandoned() {
            fence(Ordering::Acquire);
            return Err(DspPublicationError::Disconnected);
        }
        executor.reset();
        self.pending
            .push(executor)
            .map_err(|PushError::Full(_)| DspPublicationError::QueueFull)
    }

    pub fn compile_and_enqueue(
        &mut self,
        snapshot: &Snapshot,
        sources: ResidentDspSources<'a>,
    ) -> Result<(), DspPublicationError> {
        let program = CompiledDspProgram::compile(snapshot, &sources, self.sample_rate)
            .map_err(DspPublicationError::Build)?;
        let executor =
            DspExecutor::prepare(program, sources).map_err(DspPublicationError::Build)?;
        self.enqueue(executor)
    }

    /// Drops retired executors on the calling thread and returns the number reclaimed.
    pub fn reclaim(&mut self) -> usize {
        let mut reclaimed = 0;
        while self.retired.pop().is_ok() {
            reclaimed += 1;
        }
        reclaimed
    }

    /// Drops both endpoints on the calling thread after the host has quiesced the callback.
    ///
    /// `callback` must be the matching half returned by [`DspSwap::channel`]. This API does not
    /// perform an identity check.
    pub fn shutdown(self, callback: DspSwapCallback<'a>) {
        drop(callback);
        drop(self);
    }
}

impl DspSwapCallback<'_> {
    /// Applies at most one boundary replacement, then processes the active executor exactly once.
    ///
    /// An empty output buffer is still a callback boundary and can activate a queued replacement.
    pub fn process(
        &mut self,
        output: &mut [[f32; 2]],
    ) -> Result<DspProcessReport, DspProcessError> {
        self.flush_deferred();
        if self.deferred.is_none()
            && let Ok(next) = self.pending.pop()
        {
            let previous = mem::replace(&mut self.current, next);
            if let Err(PushError::Full(previous)) = self.retired.push(previous) {
                self.deferred = Some(previous);
            }
        }
        self.current.process(output)
    }

    /// Resets only the currently active executor; queued and retired executors are unchanged.
    pub fn reset(&mut self) {
        self.current.reset();
    }

    #[must_use]
    pub const fn program(&self) -> &CompiledDspProgram {
        self.current.program()
    }

    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.current.cursor()
    }

    #[must_use]
    pub const fn state(&self) -> DspProcessState {
        self.current.state()
    }

    fn flush_deferred(&mut self) {
        if let Some(previous) = self.deferred.take()
            && let Err(PushError::Full(previous)) = self.retired.push(previous)
        {
            self.deferred = Some(previous);
        }
    }
}
