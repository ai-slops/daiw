use std::sync::Arc;
use std::sync::atomic::Ordering;

use rtrb::Consumer;

use super::shared::{Shared, unpack};
use super::worker::StreamBlock;
use super::{FramePosition, WavStreamState};

const MAX_STALE_DISCARDS: usize = 8;

pub struct WavStreamCallback {
    consumer: Consumer<StreamBlock>,
    shared: Arc<Shared>,
    current: Option<StreamBlock>,
    offset: usize,
}

impl WavStreamCallback {
    pub(crate) fn new(consumer: Consumer<StreamBlock>, shared: Arc<Shared>) -> Self {
        Self {
            consumer,
            shared,
            current: None,
            offset: 0,
        }
    }

    pub fn fill_output(&mut self, output: &mut [f32]) {
        let (generation, state) = self.shared.state_snapshot();
        match state {
            WavStreamState::Paused => {
                output.fill(0.0);
                self.prepare_output();
            }
            WavStreamState::Playing => self.fill_playing(output, generation),
            WavStreamState::Ended | WavStreamState::Faulted | WavStreamState::Shutdown => {
                output.fill(0.0);
            }
        }
    }

    fn fill_playing(&mut self, output: &mut [f32], generation: super::StreamGeneration) {
        output.fill(0.0);
        let initial = self.shared.cursor.load(Ordering::Acquire);
        let (cursor_generation, mut position) = unpack(initial);
        if cursor_generation != generation {
            return;
        }
        let mut supplied = 0_usize;
        let mut stale_budget = MAX_STALE_DISCARDS;
        let mut ended = false;
        let frame_count = output.len() / 2;
        while supplied < frame_count {
            if !self.ensure_block(generation, position, &mut stale_budget) {
                self.shared.underruns.fetch_add(1, Ordering::Relaxed);
                break;
            }
            let Some(block) = self.current.as_ref() else {
                break;
            };
            if self.offset >= usize::from(block.valid) {
                if block.terminal {
                    ended = true;
                    break;
                }
                self.current = None;
                self.offset = 0;
                continue;
            }
            let target = supplied * 2;
            output[target..target + 2].copy_from_slice(&block.samples[self.offset]);
            position = block.advance(position);
            self.offset += 1;
            supplied += 1;
            if self.offset == usize::from(block.valid) && block.terminal {
                ended = true;
                break;
            }
        }
        if !output.len().is_multiple_of(2) {
            output[output.len() - 1] = 0.0;
        }
        if self.shared.commit_cursor(initial, generation, position) && ended {
            self.shared.transition_state(
                generation,
                WavStreamState::Playing,
                WavStreamState::Ended,
            );
        }
    }

    fn ensure_block(
        &mut self,
        generation: super::StreamGeneration,
        position: FramePosition,
        stale_budget: &mut usize,
    ) -> bool {
        loop {
            if let Some(block) = self.current.as_ref() {
                if block.generation == generation && (self.offset > 0 || block.start == position) {
                    return true;
                }
                if *stale_budget == 0 {
                    return false;
                }
                self.current = None;
                self.offset = 0;
                self.shared.stale_blocks.fetch_add(1, Ordering::Relaxed);
                *stale_budget -= 1;
            }
            match self.consumer.pop() {
                Ok(block) => self.current = Some(block),
                Err(_) => return false,
            }
        }
    }

    pub fn prepare_output(&mut self) -> bool {
        let (generation, _) = unpack(self.shared.cursor.load(Ordering::Acquire));
        let mut stale_budget = MAX_STALE_DISCARDS;
        while stale_budget > 0 {
            if self
                .current
                .as_ref()
                .is_some_and(|block| block.generation == generation)
            {
                return true;
            }
            if self.current.take().is_some() {
                self.offset = 0;
                self.shared.stale_blocks.fetch_add(1, Ordering::Relaxed);
                stale_budget -= 1;
                continue;
            }
            match self.consumer.pop() {
                Ok(block) if block.generation != generation => {
                    self.shared.stale_blocks.fetch_add(1, Ordering::Relaxed);
                    stale_budget -= 1;
                }
                Ok(block) => {
                    self.offset = 0;
                    self.current = Some(block);
                    return true;
                }
                Err(_) => return false,
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use rtrb::RingBuffer;

    use super::{MAX_STALE_DISCARDS, WavStreamCallback};
    use crate::wav_stream::shared::Shared;
    use crate::wav_stream::worker::StreamBlock;
    use crate::wav_stream::{FramePosition, StreamGeneration, WavStreamOptions};

    #[test]
    fn one_fill_discards_at_most_eight_stale_blocks() {
        // Given
        let shared = Arc::new(Shared::new(WavStreamOptions::default()));
        let (mut producer, consumer) = RingBuffer::new(8);
        for _ in 0..8 {
            assert!(
                producer
                    .push(StreamBlock::empty(
                        StreamGeneration::INITIAL,
                        FramePosition::new(0),
                        None,
                    ))
                    .is_ok()
            );
        }
        let mut callback = WavStreamCallback::new(consumer, Arc::clone(&shared));
        callback.current = Some(StreamBlock::empty(
            StreamGeneration::INITIAL,
            FramePosition::new(0),
            None,
        ));
        let mut budget = MAX_STALE_DISCARDS;

        // When
        let found = callback.ensure_block(StreamGeneration(1), FramePosition::new(0), &mut budget);

        // Then
        assert!(!found);
        assert_eq!(budget, 0);
        assert!(callback.current.is_some());
        assert_eq!(shared.stale_blocks.load(Ordering::Relaxed), 8);
    }
}
