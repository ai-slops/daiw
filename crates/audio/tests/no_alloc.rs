use std::alloc::System;
use std::sync::Arc;

use daiw_audio::{AudioSwap, Transport};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

#[test]
fn callback_fill_allocates_nothing() {
    // Given
    let transport = Arc::new(Transport::new(8));
    transport.play();
    let samples: Arc<[f32]> = Arc::from([0.25_f32; 8]);
    let replacement: Arc<[f32]> = Arc::from([0.5_f32; 8]);
    let (mut control, mut callback) = AudioSwap::channel(transport, samples);
    assert!(control.enqueue(replacement).is_ok());
    let mut output = [0.0_f32; 8];
    let region = Region::new(GLOBAL);

    // When
    callback.fill(&mut output);

    // Then
    let change = region.change();
    assert_eq!(change.allocations, 0);
    assert_eq!(change.deallocations, 0);
    assert_eq!(output, [0.5; 8]);
    assert_eq!(control.reclaim(), 1);
}
