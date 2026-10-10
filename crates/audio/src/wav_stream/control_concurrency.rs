use std::sync::{Arc, Barrier};

use super::shared::{Shared, lock};
use super::{
    FrameLoopRange, FramePosition, StreamGeneration, WavStreamControl, WavStreamError,
    WavStreamOptions, WavStreamState,
};

fn control(shared: Arc<Shared>, frames: u32) -> WavStreamControl {
    let worker = std::thread::spawn(|| {});
    let thread = worker.thread().clone();
    WavStreamControl::new(shared, thread, worker, frames)
}

#[test]
fn concurrent_seek_and_loop_update_preserve_loop_normalization() {
    // Given
    let shared = Arc::new(Shared::new(WavStreamOptions::default()));
    let control = Arc::new(control(Arc::clone(&shared), 16));
    let barrier = Arc::new(Barrier::new(3));
    let range = match FrameLoopRange::new(FramePosition::new(4), FramePosition::new(8), 16) {
        Ok(value) => value,
        Err(error) => std::panic::panic_any(error),
    };
    let seek_control = Arc::clone(&control);
    let seek_barrier = Arc::clone(&barrier);
    let seek = std::thread::spawn(move || {
        seek_barrier.wait();
        seek_control.seek(FramePosition::new(15))
    });
    let loop_control = Arc::clone(&control);
    let loop_barrier = Arc::clone(&barrier);
    let update_loop = std::thread::spawn(move || {
        loop_barrier.wait();
        loop_control.set_loop(Some(range))
    });

    // When
    barrier.wait();
    let seek_result = seek.join();
    let loop_result = update_loop.join();
    assert!(seek_result.is_ok());
    assert!(loop_result.is_ok());
    if let Ok(result) = seek_result {
        assert!(result.is_ok());
    }
    if let Ok(result) = loop_result {
        assert!(result.is_ok());
    }

    // Then
    assert_eq!(lock(&shared.plan).loop_range, Some(range));
    assert_eq!(control.position(), FramePosition::new(4));
}

#[test]
fn faulted_controls_return_terminal_error_without_revival() {
    // Given
    let shared = Arc::new(Shared::new(WavStreamOptions::default()));
    let control = control(Arc::clone(&shared), 16);
    assert!(shared.fault(StreamGeneration::INITIAL, WavStreamError::InvalidHeader));

    // When / Then
    assert!(matches!(control.play(), Err(WavStreamError::StreamFaulted)));
    assert!(matches!(
        control.pause(),
        Err(WavStreamError::StreamFaulted)
    ));
    assert!(matches!(control.stop(), Err(WavStreamError::StreamFaulted)));
    assert_eq!(control.state(), WavStreamState::Faulted);
}

#[test]
fn pause_succeeds_when_eof_wins_after_playing_snapshot() {
    // Given
    let shared = Arc::new(Shared::new(WavStreamOptions::default()));
    shared.publish_state(StreamGeneration::INITIAL, WavStreamState::Playing);
    let control = control(Arc::clone(&shared), 16);
    let entered = Arc::new(Barrier::new(2));
    let finished = Arc::new(Barrier::new(2));
    let callback_shared = Arc::clone(&shared);
    let callback_entered = Arc::clone(&entered);
    let callback_finished = Arc::clone(&finished);
    let callback = std::thread::spawn(move || {
        callback_entered.wait();
        callback_shared.transition_state(
            StreamGeneration::INITIAL,
            WavStreamState::Playing,
            WavStreamState::Ended,
        );
        callback_finished.wait();
    });

    // When
    let result = control.pause_with_hook(|| {
        entered.wait();
        finished.wait();
    });

    // Then
    assert!(callback.join().is_ok());
    assert!(result.is_ok());
    assert_eq!(control.state(), WavStreamState::Ended);
}

#[test]
fn seek_and_loop_succeed_when_eof_wins_after_playing_snapshot() {
    // Given
    let shared = Arc::new(Shared::new(WavStreamOptions::default()));
    shared.publish_state(StreamGeneration::INITIAL, WavStreamState::Playing);
    let control = control(Arc::clone(&shared), 16);
    let range = match FrameLoopRange::new(FramePosition::new(4), FramePosition::new(8), 16) {
        Ok(value) => value,
        Err(error) => std::panic::panic_any(error),
    };

    // When
    let seek = control.seek_with_hook(FramePosition::new(6), || {
        shared.transition_state(
            StreamGeneration::INITIAL,
            WavStreamState::Playing,
            WavStreamState::Ended,
        );
    });
    shared.publish_state(StreamGeneration(1), WavStreamState::Playing);
    let set_loop = control.set_loop_with_hook(Some(range), || {
        shared.transition_state(
            StreamGeneration(1),
            WavStreamState::Playing,
            WavStreamState::Ended,
        );
    });

    // Then
    assert!(seek.is_ok());
    assert!(set_loop.is_ok());
    assert_eq!(control.position(), FramePosition::new(6));
    assert_eq!(control.state(), WavStreamState::Paused);
    assert_eq!(lock(&shared.plan).loop_range, Some(range));
}

#[test]
fn set_loop_preserves_callback_cursor_commit_after_initial_load() {
    // Given
    let shared = Arc::new(Shared::new(WavStreamOptions::default()));
    shared.publish_state(StreamGeneration::INITIAL, WavStreamState::Playing);
    let control = control(Arc::clone(&shared), 16);
    let initial = shared.cursor.load(std::sync::atomic::Ordering::Acquire);
    let range = match FrameLoopRange::new(FramePosition::new(4), FramePosition::new(8), 16) {
        Ok(value) => value,
        Err(error) => std::panic::panic_any(error),
    };

    // When
    let result = control.set_loop_with_hook(Some(range), || {
        assert!(shared.commit_cursor(initial, StreamGeneration::INITIAL, FramePosition::new(6),));
    });

    // Then
    assert!(result.is_ok());
    assert_eq!(control.position(), FramePosition::new(6));
}

#[test]
fn set_loop_reads_cursor_after_serialized_seek_publication() {
    // Given
    let shared = Arc::new(Shared::new(WavStreamOptions::default()));
    let control = Arc::new(control(Arc::clone(&shared), 16));
    let range = match FrameLoopRange::new(FramePosition::new(4), FramePosition::new(8), 16) {
        Ok(value) => value,
        Err(error) => std::panic::panic_any(error),
    };
    let seek_entered = Arc::new(Barrier::new(2));
    let seek_release = Arc::new(Barrier::new(2));
    let seek_control = Arc::clone(&control);
    let seek_entered_thread = Arc::clone(&seek_entered);
    let seek_release_thread = Arc::clone(&seek_release);
    let seek = std::thread::spawn(move || {
        seek_control.seek_with_hook(FramePosition::new(15), || {
            seek_entered_thread.wait();
            seek_release_thread.wait();
        })
    });
    seek_entered.wait();
    let loop_prelock = Arc::new(Barrier::new(2));
    let loop_control = Arc::clone(&control);
    let loop_prelock_thread = Arc::clone(&loop_prelock);
    let update_loop = std::thread::spawn(move || {
        loop_control.set_loop_with_prelock_hook(Some(range), || {
            loop_prelock_thread.wait();
        })
    });
    loop_prelock.wait();

    // When
    seek_release.wait();
    let seek_result = seek.join();
    let loop_result = update_loop.join();

    // Then
    assert!(matches!(seek_result, Ok(Ok(()))));
    assert!(matches!(loop_result, Ok(Ok(()))));
    assert_eq!(control.position(), FramePosition::new(4));
}

#[test]
fn seek_reads_loop_after_serialized_loop_publication() {
    // Given
    let shared = Arc::new(Shared::new(WavStreamOptions::default()));
    let control = Arc::new(control(Arc::clone(&shared), 16));
    let range = match FrameLoopRange::new(FramePosition::new(4), FramePosition::new(8), 16) {
        Ok(value) => value,
        Err(error) => std::panic::panic_any(error),
    };
    let loop_entered = Arc::new(Barrier::new(2));
    let loop_release = Arc::new(Barrier::new(2));
    let loop_control = Arc::clone(&control);
    let loop_entered_thread = Arc::clone(&loop_entered);
    let loop_release_thread = Arc::clone(&loop_release);
    let update_loop = std::thread::spawn(move || {
        loop_control.set_loop_with_hook(Some(range), || {
            loop_entered_thread.wait();
            loop_release_thread.wait();
        })
    });
    loop_entered.wait();
    let seek_prelock = Arc::new(Barrier::new(2));
    let seek_control = Arc::clone(&control);
    let seek_prelock_thread = Arc::clone(&seek_prelock);
    let seek = std::thread::spawn(move || {
        seek_control.seek_with_prelock_hook(FramePosition::new(15), || {
            seek_prelock_thread.wait();
        })
    });
    seek_prelock.wait();

    // When
    loop_release.wait();
    let loop_result = update_loop.join();
    let seek_result = seek.join();

    // Then
    assert!(matches!(loop_result, Ok(Ok(()))));
    assert!(matches!(seek_result, Ok(Ok(()))));
    assert_eq!(lock(&shared.plan).loop_range, Some(range));
    assert_eq!(control.position(), FramePosition::new(4));
}
