use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use super::types::normalize_position;
use super::{FrameLoopRange, FramePosition, StreamGeneration, WavStreamError, WavStreamState};

pub(crate) struct ReadPlan {
    pub generation: StreamGeneration,
    pub position: FramePosition,
    pub loop_range: Option<FrameLoopRange>,
}

pub(crate) struct Shared {
    pub plan: Mutex<ReadPlan>,
    pub cursor: AtomicU64,
    pub lifecycle: AtomicU64,
    pub shutdown: AtomicBool,
    pub underruns: AtomicU64,
    pub stale_blocks: AtomicU64,
    pub device_errors: AtomicU64,
    pub error: Mutex<Option<WavStreamError>>,
}

impl Shared {
    pub fn new(options: super::WavStreamOptions) -> Self {
        let generation = StreamGeneration::INITIAL;
        let position = normalize_position(options.start_frame, options.loop_range);
        Self {
            plan: Mutex::new(ReadPlan {
                generation,
                position,
                loop_range: options.loop_range,
            }),
            cursor: AtomicU64::new(pack(generation, position)),
            lifecycle: AtomicU64::new(pack_state(generation, WavStreamState::Paused)),
            shutdown: AtomicBool::new(false),
            underruns: AtomicU64::new(0),
            stale_blocks: AtomicU64::new(0),
            device_errors: AtomicU64::new(0),
            error: Mutex::new(None),
        }
    }

    pub fn state(&self) -> WavStreamState {
        unpack_state(self.lifecycle.load(Ordering::Acquire)).1
    }

    pub fn state_snapshot(&self) -> (StreamGeneration, WavStreamState) {
        unpack_state(self.lifecycle.load(Ordering::Acquire))
    }

    pub fn publish_state(&self, generation: StreamGeneration, state: WavStreamState) {
        self.lifecycle
            .store(pack_state(generation, state), Ordering::Release);
    }

    pub fn transition_state(
        &self,
        generation: StreamGeneration,
        from: WavStreamState,
        to: WavStreamState,
    ) -> bool {
        self.lifecycle
            .compare_exchange(
                pack_state(generation, from),
                pack_state(generation, to),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    pub fn transition_generation(
        &self,
        from_generation: StreamGeneration,
        from_state: WavStreamState,
        to_generation: StreamGeneration,
        to_state: WavStreamState,
    ) -> bool {
        self.lifecycle
            .compare_exchange(
                pack_state(from_generation, from_state),
                pack_state(to_generation, to_state),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    pub fn fault(&self, generation: StreamGeneration, error: WavStreamError) -> bool {
        let mut slot = lock(&self.error);
        loop {
            let current = self.lifecycle.load(Ordering::Acquire);
            let (current_generation, state) = unpack_state(current);
            if current_generation != generation
                || matches!(state, WavStreamState::Faulted | WavStreamState::Shutdown)
            {
                return false;
            }
            if self
                .lifecycle
                .compare_exchange(
                    current,
                    pack_state(generation, WavStreamState::Faulted),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                if slot.is_none() {
                    *slot = Some(error);
                }
                return true;
            }
        }
    }

    pub fn commit_cursor(
        &self,
        expected: u64,
        generation: StreamGeneration,
        position: FramePosition,
    ) -> bool {
        self.cursor
            .compare_exchange(
                expected,
                pack(generation, position),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }
}

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub(crate) const fn pack(generation: StreamGeneration, position: FramePosition) -> u64 {
    (generation.get() as u64) << 32 | position.get() as u64
}

pub(crate) const fn unpack(value: u64) -> (StreamGeneration, FramePosition) {
    (
        StreamGeneration((value >> 32) as u32),
        FramePosition::new(value as u32),
    )
}

pub(crate) const fn pack_state(generation: StreamGeneration, state: WavStreamState) -> u64 {
    (generation.get() as u64) << 32
        | match state {
            WavStreamState::Paused => 0,
            WavStreamState::Playing => 1,
            WavStreamState::Ended => 2,
            WavStreamState::Faulted => 3,
            WavStreamState::Shutdown => 4,
        }
}

pub(crate) const fn unpack_state(value: u64) -> (StreamGeneration, WavStreamState) {
    let state = match value as u32 {
        0 => WavStreamState::Paused,
        1 => WavStreamState::Playing,
        2 => WavStreamState::Ended,
        3 => WavStreamState::Faulted,
        _ => WavStreamState::Shutdown,
    };
    (StreamGeneration((value >> 32) as u32), state)
}
