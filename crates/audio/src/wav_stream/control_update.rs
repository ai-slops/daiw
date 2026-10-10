use std::sync::atomic::Ordering;

use super::control::WavStreamControl;
use super::shared::{lock, pack, unpack};
use super::types::normalize_position;
use super::{FrameLoopRange, FramePosition, WavStreamError, WavStreamState};

impl WavStreamControl {
    pub fn stop(&self) -> Result<(), WavStreamError> {
        self.update_explicit(
            ExplicitUpdate {
                position: FramePosition::new(0),
                mode: UpdateMode::Pause,
            },
            || {},
        )
    }

    pub fn seek(&self, position: FramePosition) -> Result<(), WavStreamError> {
        self.validate_position(position)?;
        self.update_explicit(
            ExplicitUpdate {
                position,
                mode: UpdateMode::Resume,
            },
            || {},
        )
    }

    pub fn set_loop(&self, loop_range: Option<FrameLoopRange>) -> Result<(), WavStreamError> {
        self.validate_loop(loop_range)?;
        self.update_loop(loop_range, || {}, || {})
    }

    #[cfg(test)]
    pub(crate) fn seek_with_hook(
        &self,
        position: FramePosition,
        hook: impl FnOnce(),
    ) -> Result<(), WavStreamError> {
        self.validate_position(position)?;
        self.update_explicit(
            ExplicitUpdate {
                position,
                mode: UpdateMode::Resume,
            },
            hook,
        )
    }

    #[cfg(test)]
    pub(crate) fn seek_with_prelock_hook(
        &self,
        position: FramePosition,
        hook: impl FnOnce(),
    ) -> Result<(), WavStreamError> {
        self.validate_position(position)?;
        hook();
        self.update_explicit(
            ExplicitUpdate {
                position,
                mode: UpdateMode::Resume,
            },
            || {},
        )
    }

    #[cfg(test)]
    pub(crate) fn set_loop_with_hook(
        &self,
        loop_range: Option<FrameLoopRange>,
        hook: impl FnOnce(),
    ) -> Result<(), WavStreamError> {
        self.validate_loop(loop_range)?;
        self.update_loop(loop_range, || {}, hook)
    }

    #[cfg(test)]
    pub(crate) fn set_loop_with_prelock_hook(
        &self,
        loop_range: Option<FrameLoopRange>,
        hook: impl FnOnce(),
    ) -> Result<(), WavStreamError> {
        self.validate_loop(loop_range)?;
        self.update_loop(loop_range, hook, || {})
    }

    fn update_explicit(
        &self,
        update: ExplicitUpdate,
        hook: impl FnOnce(),
    ) -> Result<(), WavStreamError> {
        let mut plan = lock(&self.shared.plan);
        let generation = plan.generation.next()?;
        let position = normalize_position(update.position, plan.loop_range);
        self.transition_generation(
            &plan,
            Transition {
                generation,
                mode: update.mode,
            },
            hook,
        )?;
        plan.generation = generation;
        plan.position = position;
        self.shared
            .cursor
            .store(pack(generation, position), Ordering::Release);
        self.worker_thread.unpark();
        Ok(())
    }

    fn update_loop(
        &self,
        loop_range: Option<FrameLoopRange>,
        prelock_hook: impl FnOnce(),
        cursor_hook: impl FnOnce(),
    ) -> Result<(), WavStreamError> {
        prelock_hook();
        let mut plan = lock(&self.shared.plan);
        let generation = plan.generation.next()?;
        let mut cursor = self.shared.cursor.load(Ordering::Acquire);
        self.transition_generation(
            &plan,
            Transition {
                generation,
                mode: UpdateMode::Resume,
            },
            cursor_hook,
        )?;
        let position = loop {
            let (cursor_generation, position) = unpack(cursor);
            debug_assert_eq!(cursor_generation, plan.generation);
            let normalized = normalize_position(position, loop_range);
            match self.shared.cursor.compare_exchange(
                cursor,
                pack(generation, normalized),
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break normalized,
                Err(latest) => cursor = latest,
            }
        };
        plan.generation = generation;
        plan.position = position;
        plan.loop_range = loop_range;
        self.worker_thread.unpark();
        Ok(())
    }

    fn transition_generation(
        &self,
        plan: &super::shared::ReadPlan,
        transition: Transition,
        hook: impl FnOnce(),
    ) -> Result<(), WavStreamError> {
        let mut hook = Some(hook);
        loop {
            let previous = self.operational_state()?;
            let state = transition.mode.state(previous);
            if let Some(run) = hook.take() {
                run();
            }
            if self.shared.transition_generation(
                plan.generation,
                previous,
                transition.generation,
                state,
            ) {
                return Ok(());
            }
        }
    }

    fn validate_position(&self, position: FramePosition) -> Result<(), WavStreamError> {
        if position.get() > self.frames {
            return Err(WavStreamError::InvalidStartFrame);
        }
        Ok(())
    }

    fn validate_loop(&self, loop_range: Option<FrameLoopRange>) -> Result<(), WavStreamError> {
        if loop_range.is_some_and(|range| range.end().get() > self.frames) {
            return Err(WavStreamError::InvalidLoopRange);
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct ExplicitUpdate {
    position: FramePosition,
    mode: UpdateMode,
}

#[derive(Clone, Copy)]
struct Transition {
    generation: super::StreamGeneration,
    mode: UpdateMode,
}

#[derive(Clone, Copy)]
enum UpdateMode {
    Pause,
    Resume,
}

impl UpdateMode {
    const fn state(self, current: WavStreamState) -> WavStreamState {
        match self {
            Self::Pause => WavStreamState::Paused,
            Self::Resume => match current {
                WavStreamState::Ended => WavStreamState::Paused,
                WavStreamState::Paused => WavStreamState::Paused,
                WavStreamState::Playing => WavStreamState::Playing,
                WavStreamState::Faulted => WavStreamState::Faulted,
                WavStreamState::Shutdown => WavStreamState::Shutdown,
            },
        }
    }
}
