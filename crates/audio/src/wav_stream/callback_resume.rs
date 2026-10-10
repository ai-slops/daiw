use std::sync::Arc;
use std::sync::atomic::Ordering;

use rtrb::RingBuffer;

use super::shared::{Shared, pack};
use super::worker::StreamBlock;
use super::{FramePosition, StreamGeneration, WavStreamCallback, WavStreamOptions, WavStreamState};

fn sampled_block(
    generation: StreamGeneration,
    start: FramePosition,
    first_sample: i16,
) -> StreamBlock {
    let mut block = StreamBlock::empty(generation, start, None);
    for (index, frame) in block.samples[..16].iter_mut().enumerate() {
        let sample = first_sample + i16::try_from(index).unwrap_or_default() * 100;
        *frame = [f32::from(sample) / 32_768.0; 2];
    }
    block.valid = 16;
    block.next = FramePosition::new(start.get() + 16);
    block
}

#[test]
fn paused_seek_discards_old_partial_offset_before_cross_boundary_resume() {
    // Given
    let shared = Arc::new(Shared::new(WavStreamOptions::default()));
    let (mut producer, consumer) = RingBuffer::new(2);
    assert!(
        producer
            .push(sampled_block(
                StreamGeneration::INITIAL,
                FramePosition::new(0),
                0,
            ))
            .is_ok()
    );
    let mut callback = WavStreamCallback::new(consumer, Arc::clone(&shared));
    shared.publish_state(StreamGeneration::INITIAL, WavStreamState::Playing);
    let mut initial = [0.0_f32; 16];
    callback.fill_output(&mut initial);
    shared.publish_state(StreamGeneration::INITIAL, WavStreamState::Paused);
    let mut paused = [1.0_f32; 2];
    callback.fill_output(&mut paused);
    let next_generation = StreamGeneration(1);
    shared.cursor.store(
        pack(next_generation, FramePosition::new(4095)),
        Ordering::Release,
    );
    shared.publish_state(next_generation, WavStreamState::Paused);
    assert!(
        producer
            .push(sampled_block(
                next_generation,
                FramePosition::new(4095),
                9_600,
            ))
            .is_ok()
    );
    callback.fill_output(&mut paused);

    // When
    shared.publish_state(next_generation, WavStreamState::Playing);
    let mut resumed = [0.0_f32; 4];
    callback.fill_output(&mut resumed);

    // Then
    assert_eq!(paused, [0.0; 2]);
    assert_eq!(resumed[0], 0.292_968_75);
    assert_eq!(resumed[2], 9_700.0 / 32_768.0);
    assert_eq!(
        super::shared::unpack(shared.cursor.load(Ordering::Acquire)).1,
        FramePosition::new(4097)
    );
}

#[test]
fn pause_without_seek_retains_valid_partial_offset() {
    // Given
    let shared = Arc::new(Shared::new(WavStreamOptions::default()));
    let (mut producer, consumer) = RingBuffer::new(1);
    assert!(
        producer
            .push(sampled_block(
                StreamGeneration::INITIAL,
                FramePosition::new(0),
                0,
            ))
            .is_ok()
    );
    let mut callback = WavStreamCallback::new(consumer, Arc::clone(&shared));
    shared.publish_state(StreamGeneration::INITIAL, WavStreamState::Playing);
    let mut initial = [0.0_f32; 16];
    callback.fill_output(&mut initial);
    shared.publish_state(StreamGeneration::INITIAL, WavStreamState::Paused);
    let mut paused = [1.0_f32; 2];
    callback.fill_output(&mut paused);

    // When
    shared.publish_state(StreamGeneration::INITIAL, WavStreamState::Playing);
    let mut resumed = [0.0_f32; 2];
    callback.fill_output(&mut resumed);

    // Then
    assert_eq!(paused, [0.0; 2]);
    assert_eq!(resumed, [800.0 / 32_768.0; 2]);
}
