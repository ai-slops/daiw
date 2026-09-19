use std::sync::Arc;

use daiw_audio::{AudioSwap, CallbackBuffer, LoopRange, Transport};

#[path = "support/must.rs"]
mod support;
use support::must;

#[test]
fn transport_seek_and_loop_preserve_exact_sample_offsets() {
    // Given
    let transport = Transport::new(16);
    transport.set_loop(Some(must(LoopRange::new(4, 8))));
    transport.seek(6);
    transport.play();
    let mut player = CallbackBuffer::new(&transport, &[0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0]);
    let mut output = [0.0; 8];

    // When
    player.fill(&mut output);

    // Then
    assert_eq!(output, [3.0, 3.0, 2.0, 2.0, 3.0, 3.0, 2.0, 2.0]);
    assert_eq!(transport.position(), 6);
}

#[test]
fn paused_callback_fills_silence_without_advancing() {
    // Given
    let transport = Transport::new(4);
    transport.seek(2);
    let mut player = CallbackBuffer::new(&transport, &[0.5; 8]);
    let mut output = [1.0; 4];

    // When
    player.fill(&mut output);

    // Then
    assert_eq!(output, [0.0; 4]);
    assert_eq!(transport.position(), 2);
}

#[test]
fn audio_swap_replaces_buffer_at_callback_boundary_and_reclaims_on_control_thread() {
    // Given
    let transport = Arc::new(Transport::new(4));
    transport.play();
    let (mut control, mut callback) =
        AudioSwap::channel(Arc::clone(&transport), Arc::from([0.25; 4]));
    must(control.enqueue(Arc::from([0.75; 4])));
    let mut output = [0.0; 4];

    // When
    callback.fill(&mut output);
    let reclaimed = control.reclaim();

    // Then
    assert_eq!(output, [0.75; 4]);
    assert_eq!(reclaimed, 1);
}
