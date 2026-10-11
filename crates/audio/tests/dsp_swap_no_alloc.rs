use std::alloc::System;

use daiw_audio::{
    DspProcessError, DspProcessReport, DspPublicationError, DspSwap, DspSwapCallback,
};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};

#[path = "support/dsp_swap_fixture.rs"]
mod fixture;
use fixture::{assets, executor, executor_at_rate, fault_executor};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

fn process_no_alloc(
    callback: &mut DspSwapCallback<'_>,
    output: &mut [[f32; 2]],
) -> Result<DspProcessReport, DspProcessError> {
    let region = Region::new(GLOBAL);
    let result = callback.process(output);
    let change = region.change();
    assert_eq!(change.allocations, 0);
    assert_eq!(change.deallocations, 0);
    result
}

#[test]
fn every_callback_process_path_has_zero_allocations_and_deallocations() {
    // Given
    let a = assets(0.1, 2);
    let b = assets(0.2, 2);
    let c = assets(0.3, 2);
    let d = assets(0.4, 2);
    let fault = assets(f32::MAX, 2);
    let wrong_rate = assets(0.5, 2);
    let (mut control, mut callback) = DspSwap::channel(executor("a", &a, 2));
    assert!(matches!(
        control.enqueue(executor_at_rate("wrong", &wrong_rate, 2, 44_100)),
        Err(DspPublicationError::SampleRateMismatch { .. })
    ));
    let mut output = [[0.0; 2]; 2];
    let mut empty = [];

    // When
    assert!(process_no_alloc(&mut callback, &mut empty).is_ok());
    assert!(process_no_alloc(&mut callback, &mut output).is_ok());
    assert!(process_no_alloc(&mut callback, &mut output).is_ok());
    let reset_region = Region::new(GLOBAL);
    callback.reset();
    assert_eq!(reset_region.change().allocations, 0);
    assert_eq!(reset_region.change().deallocations, 0);
    assert!(control.enqueue(executor("b", &b, 2)).is_ok());
    assert!(process_no_alloc(&mut callback, &mut output[..1]).is_ok());
    assert!(control.enqueue(executor("c", &c, 2)).is_ok());
    assert!(process_no_alloc(&mut callback, &mut output[..1]).is_ok());
    assert!(control.enqueue(executor("d", &d, 2)).is_ok());
    assert!(process_no_alloc(&mut callback, &mut empty).is_ok());
    assert_eq!(control.reclaim(), 1);
    assert!(process_no_alloc(&mut callback, &mut empty).is_ok());
    assert_eq!(control.reclaim(), 1);
    assert!(control.enqueue(fault_executor(&fault)).is_ok());
    assert!(process_no_alloc(&mut callback, &mut output).is_err());
    assert!(process_no_alloc(&mut callback, &mut output).is_err());
    callback.reset();
    assert!(process_no_alloc(&mut callback, &mut output).is_err());
}
