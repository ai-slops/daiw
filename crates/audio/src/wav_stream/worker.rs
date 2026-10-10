use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use rtrb::{Producer, PushError};

use super::shared::{Shared, lock};
use super::{FrameLoopRange, FramePosition, STREAM_BLOCK_FRAMES, StreamGeneration, WavStreamError};

const PAYLOAD_OFFSET: u64 = 44;
const FRAME_BYTES: u64 = 4;
const NO_LOOP: u32 = u32::MAX;

#[derive(Clone, Copy)]
pub(crate) struct StreamBlock {
    pub samples: [[f32; 2]; STREAM_BLOCK_FRAMES],
    pub generation: StreamGeneration,
    pub start: FramePosition,
    pub next: FramePosition,
    pub valid: u16,
    pub loop_start: u32,
    pub loop_end: u32,
    pub terminal: bool,
}

impl StreamBlock {
    pub(crate) fn empty(
        generation: StreamGeneration,
        start: FramePosition,
        range: Option<FrameLoopRange>,
    ) -> Self {
        let (loop_start, loop_end) = match range {
            Some(value) => (value.start().get(), value.end().get()),
            None => (NO_LOOP, NO_LOOP),
        };
        Self {
            samples: [[0.0; 2]; STREAM_BLOCK_FRAMES],
            generation,
            start,
            next: start,
            valid: 0,
            loop_start,
            loop_end,
            terminal: false,
        }
    }

    pub const fn advance(&self, position: FramePosition) -> FramePosition {
        let next = position.get() + 1;
        if self.loop_end != NO_LOOP && next >= self.loop_end {
            FramePosition::new(self.loop_start)
        } else {
            FramePosition::new(next)
        }
    }
}

pub(crate) fn run_worker(
    mut file: File,
    frames: u32,
    mut producer: Producer<StreamBlock>,
    shared: Arc<Shared>,
    ready: std::sync::mpsc::SyncSender<()>,
) {
    let mut announced = false;
    let mut produced_generation = None;
    let mut position = FramePosition::new(0);
    let mut queued = 0_usize;
    loop {
        if shared.shutdown.load(Ordering::Acquire) {
            return;
        }
        let (generation, requested, loop_range) = {
            let plan = lock(&shared.plan);
            (plan.generation, plan.position, plan.loop_range)
        };
        if produced_generation != Some(generation) {
            produced_generation = Some(generation);
            position = super::types::normalize_position(requested, loop_range);
            queued = 0;
        }
        let mut block = StreamBlock::empty(generation, position, loop_range);
        match fill_block(&mut file, frames, &mut block) {
            Ok(()) => {}
            Err(error) => {
                if lock(&shared.plan).generation == generation && shared.fault(generation, error) {
                    let _ = ready.try_send(());
                    return;
                }
                continue;
            }
        }
        if lock(&shared.plan).generation != generation {
            continue;
        }
        loop {
            match producer.push(block) {
                Ok(()) => break,
                Err(PushError::Full(returned)) => {
                    block = returned;
                    std::thread::park_timeout(Duration::from_millis(1));
                    if shared.shutdown.load(Ordering::Acquire)
                        || lock(&shared.plan).generation != generation
                    {
                        break;
                    }
                }
            }
        }
        if lock(&shared.plan).generation != generation {
            continue;
        }
        queued += 1;
        if !announced && (block.terminal || queued == super::STREAM_RING_BLOCKS) {
            announced = true;
            let _ = ready.try_send(());
        }
        position = block.next;
        if block.terminal {
            while !shared.shutdown.load(Ordering::Acquire)
                && lock(&shared.plan).generation == generation
            {
                std::thread::park_timeout(Duration::from_millis(1));
            }
        }
    }
}

fn fill_block(file: &mut File, frames: u32, block: &mut StreamBlock) -> Result<(), WavStreamError> {
    let mut bytes = [0_u8; STREAM_BLOCK_FRAMES * 4];
    let mut position = block.start;
    let mut written = 0_usize;
    while written < STREAM_BLOCK_FRAMES {
        position = normalize_raw(position, block.loop_start, block.loop_end);
        let boundary = if block.loop_end == NO_LOOP {
            frames
        } else {
            block.loop_end
        };
        if position.get() >= boundary {
            block.terminal = true;
            break;
        }
        let available = usize::try_from(boundary - position.get())?;
        let count = available.min(STREAM_BLOCK_FRAMES - written);
        let offset = PAYLOAD_OFFSET + u64::from(position.get()) * FRAME_BYTES;
        file.seek(SeekFrom::Start(offset))?;
        file.read_exact(&mut bytes[..count * 4])?;
        for (target, encoded) in block.samples[written..written + count]
            .iter_mut()
            .zip(bytes[..count * 4].chunks_exact(4))
        {
            target[0] = f32::from(i16::from_le_bytes([encoded[0], encoded[1]])) / 32_768.0;
            target[1] = f32::from(i16::from_le_bytes([encoded[2], encoded[3]])) / 32_768.0;
        }
        written += count;
        position = FramePosition::new(position.get() + u32::try_from(count)?);
        if block.loop_end == NO_LOOP && position.get() == frames {
            block.terminal = true;
            break;
        }
    }
    block.valid = u16::try_from(written)?;
    block.next = normalize_raw(position, block.loop_start, block.loop_end);
    Ok(())
}

const fn normalize_raw(position: FramePosition, start: u32, end: u32) -> FramePosition {
    if end != NO_LOOP && position.get() >= end {
        FramePosition::new(start)
    } else {
        position
    }
}
