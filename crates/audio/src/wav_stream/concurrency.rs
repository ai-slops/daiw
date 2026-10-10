use std::sync::atomic::Ordering;
use std::sync::{Arc, Barrier};

use rtrb::RingBuffer;

use super::shared::{Shared, lock, pack, unpack};
use super::worker::StreamBlock;
use super::{
    FramePosition, StreamGeneration, WavStreamCallback, WavStreamControl, WavStreamOptions,
    WavStreamState,
};

#[test]
fn seek_generation_survives_old_callback_commit() {
    // Given
    let shared = Shared::new(WavStreamOptions::default());
    let initial = shared.cursor.load(Ordering::Acquire);
    let next_generation = StreamGeneration(1);
    shared.cursor.store(
        pack(next_generation, FramePosition::new(20)),
        Ordering::Release,
    );

    // When
    shared.commit_cursor(initial, StreamGeneration::INITIAL, FramePosition::new(8));

    // Then
    assert_eq!(
        unpack(shared.cursor.load(Ordering::Acquire)),
        (next_generation, FramePosition::new(20))
    );
}

#[test]
fn old_eof_cannot_end_new_playing_generation() {
    // Given
    let shared = Arc::new(Shared::new(WavStreamOptions::default()));
    shared.publish_state(StreamGeneration::INITIAL, super::WavStreamState::Playing);
    let callback_cursor = shared.cursor.load(Ordering::Acquire);
    let ready = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let callback_shared = Arc::clone(&shared);
    let callback_ready = Arc::clone(&ready);
    let callback_release = Arc::clone(&release);
    let callback = std::thread::spawn(move || {
        callback_ready.wait();
        callback_release.wait();
        callback_shared.commit_cursor(
            callback_cursor,
            StreamGeneration::INITIAL,
            FramePosition::new(8),
        );
        callback_shared.transition_state(
            StreamGeneration::INITIAL,
            super::WavStreamState::Playing,
            super::WavStreamState::Ended,
        );
    });
    ready.wait();
    let next = StreamGeneration(1);
    shared
        .cursor
        .store(pack(next, FramePosition::new(20)), Ordering::Release);
    shared.publish_state(next, super::WavStreamState::Playing);

    // When
    release.wait();
    assert!(callback.join().is_ok());

    // Then
    assert_eq!(shared.state(), super::WavStreamState::Playing);
    assert_eq!(
        unpack(shared.cursor.load(Ordering::Acquire)),
        (next, FramePosition::new(20))
    );
}

#[test]
fn stale_worker_error_cannot_fault_new_generation() {
    // Given
    let shared = Arc::new(Shared::new(WavStreamOptions::default()));
    shared.publish_state(StreamGeneration::INITIAL, super::WavStreamState::Playing);
    let ready = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let worker_shared = Arc::clone(&shared);
    let worker_ready = Arc::clone(&ready);
    let worker_release = Arc::clone(&release);
    let faulted = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let worker_faulted = Arc::clone(&faulted);
    let worker = std::thread::spawn(move || {
        worker_ready.wait();
        worker_release.wait();
        worker_faulted.store(
            worker_shared.fault(
                StreamGeneration::INITIAL,
                super::WavStreamError::InvalidHeader,
            ),
            Ordering::Release,
        );
    });
    ready.wait();
    shared.cursor.store(
        pack(StreamGeneration(1), FramePosition::new(4)),
        Ordering::Release,
    );
    shared.publish_state(StreamGeneration(1), super::WavStreamState::Playing);

    // When
    release.wait();
    assert!(worker.join().is_ok());

    // Then
    assert!(!faulted.load(Ordering::Acquire));
    assert_eq!(shared.state(), super::WavStreamState::Playing);
    assert!(lock(&shared.error).is_none());
}

#[test]
fn lifecycle_cursor_generation_mismatch_returns_silence_without_underrun() {
    // Given
    let shared = Arc::new(Shared::new(WavStreamOptions::default()));
    let (_producer, consumer) = RingBuffer::<StreamBlock>::new(1);
    let mut callback = WavStreamCallback::new(consumer, Arc::clone(&shared));
    shared.publish_state(StreamGeneration(1), super::WavStreamState::Playing);
    let mut output = [1.0_f32; 4];

    // When
    callback.fill_output(&mut output);

    // Then
    assert_eq!(output, [0.0; 4]);
    assert_eq!(shared.underruns.load(Ordering::Relaxed), 0);
}

#[test]
fn shutdown_allows_inflight_cursor_commit_without_state_revival() {
    let shared = Arc::new(Shared::new(WavStreamOptions::default()));
    shared.publish_state(StreamGeneration::INITIAL, WavStreamState::Playing);
    let initial = shared.cursor.load(Ordering::Acquire);
    let ready = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let callback_shared = Arc::clone(&shared);
    let callback_ready = Arc::clone(&ready);
    let callback_release = Arc::clone(&release);
    let callback = std::thread::spawn(move || {
        callback_ready.wait();
        callback_release.wait();
        callback_shared.commit_cursor(initial, StreamGeneration::INITIAL, FramePosition::new(8));
        callback_shared.transition_state(
            StreamGeneration::INITIAL,
            WavStreamState::Playing,
            WavStreamState::Ended,
        );
    });
    let worker_shared = Arc::clone(&shared);
    let worker = std::thread::spawn(move || {
        while !worker_shared.shutdown.load(Ordering::Acquire) {
            std::thread::park();
        }
    });
    let worker_thread = worker.thread().clone();
    let mut control = WavStreamControl::new(Arc::clone(&shared), worker_thread, worker, 16);
    ready.wait();

    let shutdown_result = control.shutdown();
    release.wait();
    let callback_result = callback.join();

    assert!(shutdown_result.is_ok());
    assert!(callback_result.is_ok());
    assert_eq!(shared.state(), WavStreamState::Shutdown);
    assert_eq!(
        unpack(shared.cursor.load(Ordering::Acquire)),
        (StreamGeneration::INITIAL, FramePosition::new(8))
    );
}
