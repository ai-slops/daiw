use std::{
    mem,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use rtrb::{Consumer, Producer, PushError, RingBuffer};

use crate::{AudioError, loop_state::LoopState};

const NO_PENDING_SEEK: usize = usize::MAX;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoopRange {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl LoopRange {
    pub const fn new(start: usize, end: usize) -> Result<Self, AudioError> {
        if start >= end {
            return Err(AudioError::InvalidLoop { start, end });
        }
        Ok(Self { start, end })
    }
}

#[derive(Debug)]
pub struct Transport {
    playing: AtomicBool,
    position: AtomicUsize,
    pending_seek: AtomicUsize,
    length: AtomicUsize,
    loops: LoopState,
}

impl Transport {
    #[must_use]
    pub const fn new(length: usize) -> Self {
        Self {
            playing: AtomicBool::new(false),
            position: AtomicUsize::new(0),
            pending_seek: AtomicUsize::new(NO_PENDING_SEEK),
            length: AtomicUsize::new(length),
            loops: LoopState::new(),
        }
    }

    pub fn play(&self) {
        self.playing.store(true, Ordering::SeqCst);
    }

    pub fn pause(&self) {
        self.playing.store(false, Ordering::SeqCst);
    }

    pub fn seek(&self, sample_offset: usize) {
        self.pending_seek
            .store(sample_offset.min(self.length()), Ordering::SeqCst);
    }

    #[must_use]
    pub fn position(&self) -> usize {
        let pending = self.pending_seek.load(Ordering::SeqCst);
        if pending == NO_PENDING_SEEK {
            self.position.load(Ordering::SeqCst)
        } else {
            pending
        }
    }

    pub fn set_loop(&self, range: Option<LoopRange>) {
        self.loops.publish(range, self.length());
    }

    fn next_position(&self, position: usize, loop_range: Option<LoopRange>) -> usize {
        if let Some(range) = loop_range
            && position >= range.end
        {
            return range.start;
        }
        position.min(self.length())
    }

    pub(crate) fn begin_block_position(&self) -> usize {
        let pending = self.pending_seek.swap(NO_PENDING_SEEK, Ordering::SeqCst);
        if pending == NO_PENDING_SEEK {
            self.position.load(Ordering::SeqCst)
        } else {
            pending
        }
    }

    pub(crate) fn commit_block_position(&self, position: usize) {
        self.position.store(position, Ordering::SeqCst);
    }

    #[must_use]
    pub fn length(&self) -> usize {
        self.length.load(Ordering::SeqCst)
    }

    fn set_length(&self, length: usize) {
        self.length.store(length, Ordering::SeqCst);
        self.seek(self.position());
    }
}

pub struct AudioSwap;

pub struct AudioSwapControl {
    pending: Producer<Arc<[f32]>>,
    retired: Consumer<Arc<[f32]>>,
}

pub struct AudioSwapCallback {
    transport: Arc<Transport>,
    current: Arc<[f32]>,
    pending: Consumer<Arc<[f32]>>,
    retired: Producer<Arc<[f32]>>,
    deferred: Option<Arc<[f32]>>,
}

impl AudioSwap {
    #[must_use]
    pub fn channel(
        transport: Arc<Transport>,
        initial: Arc<[f32]>,
    ) -> (AudioSwapControl, AudioSwapCallback) {
        let (pending_producer, pending_consumer) = RingBuffer::new(1);
        let (retired_producer, retired_consumer) = RingBuffer::new(1);
        (
            AudioSwapControl {
                pending: pending_producer,
                retired: retired_consumer,
            },
            AudioSwapCallback {
                transport,
                current: initial,
                pending: pending_consumer,
                retired: retired_producer,
                deferred: None,
            },
        )
    }
}

impl AudioSwapControl {
    pub fn enqueue(&mut self, samples: Arc<[f32]>) -> Result<(), AudioError> {
        self.reclaim();
        self.pending
            .push(samples)
            .map_err(|PushError::Full(_)| AudioError::SwapBusy)
    }

    pub fn reclaim(&mut self) -> usize {
        let mut reclaimed = 0;
        while self.retired.pop().is_ok() {
            reclaimed += 1;
        }
        reclaimed
    }
}

impl AudioSwapCallback {
    pub fn fill(&mut self, output: &mut [f32]) {
        self.flush_deferred();
        if self.deferred.is_none()
            && let Ok(next) = self.pending.pop()
        {
            let previous = mem::replace(&mut self.current, next);
            self.transport.set_length(self.current.len());
            if let Err(PushError::Full(previous)) = self.retired.push(previous) {
                self.deferred = Some(previous);
            }
        }
        CallbackBuffer::new(&self.transport, &self.current).fill(output);
    }

    fn flush_deferred(&mut self) {
        if let Some(previous) = self.deferred.take()
            && let Err(PushError::Full(previous)) = self.retired.push(previous)
        {
            self.deferred = Some(previous);
        }
    }
}

/// Allocation-free view used directly by native output callbacks.
pub struct CallbackBuffer<'a> {
    transport: &'a Transport,
    samples: &'a [f32],
}

impl<'a> CallbackBuffer<'a> {
    #[must_use]
    pub const fn new(transport: &'a Transport, samples: &'a [f32]) -> Self {
        Self { transport, samples }
    }

    pub fn fill(&mut self, output: &mut [f32]) {
        if !self.transport.playing.load(Ordering::SeqCst) {
            output.fill(0.0);
            return;
        }
        let mut position = self.transport.begin_block_position();
        let loop_range = self.transport.loops.snapshot();
        for target in output {
            position = self.transport.next_position(position, loop_range);
            if let Some(sample) = self.samples.get(position) {
                *target = *sample;
                position += 1;
            } else {
                *target = 0.0;
                self.transport.pause();
            }
        }
        self.transport.commit_block_position(position);
    }
}
