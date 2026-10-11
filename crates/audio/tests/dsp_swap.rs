use daiw_audio::{
    AudioError, DspProcessError, DspProcessState, DspPublicationError, DspSwap, RenderRequest,
    ResidentDspSources, render_with_sample_rate,
};

#[path = "support/dsp_swap_fixture.rs"]
mod fixture;
use fixture::{RATE, assets, executor, executor_at_rate, fault_executor, snapshot};

#[test]
fn publication_activates_fresh_program_at_callback_boundary() {
    // Given
    let initial_assets = assets(0.1, 5_000);
    let candidate_assets = assets(0.7, 5_000);
    let mut initial = executor("a", &initial_assets, 5_000);
    let mut candidate = executor("b", &candidate_assets, 5_000);
    let mut advance = [[0.0; 2]; 3];
    assert!(initial.process(&mut advance).is_ok());
    assert!(candidate.process(&mut advance).is_ok());
    let (mut control, mut callback) = DspSwap::channel(initial);
    let mut first = [[0.0; 2]; 1];
    let mut second = [[0.0; 2]; 5_000];
    let expected_initial = render_with_sample_rate(
        &RenderRequest {
            snapshot: snapshot("a", 5_000),
            assets: initial_assets.clone(),
        },
        RATE,
    )
    .unwrap_or_else(|error| panic!("offline render failed: {error}"));
    let expected_candidate = render_with_sample_rate(
        &RenderRequest {
            snapshot: snapshot("b", 5_000),
            assets: candidate_assets.clone(),
        },
        RATE,
    )
    .unwrap_or_else(|error| panic!("offline render failed: {error}"));

    // When
    let initial_report = callback.process(&mut first);
    let queued = control.enqueue(candidate);
    assert_eq!(callback.program().revision(), "a");
    let candidate_report = callback.process(&mut second);

    // Then
    assert!(initial_report.is_ok());
    assert!(queued.is_ok());
    assert_eq!(callback.program().revision(), "b");
    assert_eq!(callback.cursor(), 5_000);
    assert_eq!(callback.state(), DspProcessState::Ended);
    assert_eq!(candidate_report.map(|report| report.frames), Ok(5_000));
    assert_eq!(first.as_flattened(), &expected_initial.samples[..2]);
    assert_eq!(second.as_flattened(), expected_candidate.samples);
}

#[test]
fn compile_and_prepare_failures_preserve_active_and_pending_programs() {
    // Given
    let active_assets = assets(0.1, 4);
    let pending_assets = assets(0.4, 4);
    let invalid_assets = assets(f32::NAN, 4);
    let (mut control, mut callback) = DspSwap::channel(executor("active", &active_assets, 4));
    assert!(
        control
            .enqueue(executor("pending", &pending_assets, 4))
            .is_ok()
    );
    let mut invalid = snapshot("invalid-compile", 4);
    invalid.length = f64::NAN;

    // When
    let compile_error =
        control.compile_and_enqueue(&invalid, ResidentDspSources::new(&active_assets));
    let prepare_error = control.compile_and_enqueue(
        &snapshot("invalid-prepare", 4),
        ResidentDspSources::new(&invalid_assets),
    );
    assert_eq!(callback.program().revision(), "active");
    assert_eq!(callback.cursor(), 0);
    let processed = callback.process(&mut [[0.0; 2]; 1]);

    // Then
    assert!(matches!(
        compile_error,
        Err(DspPublicationError::Build(AudioError::InvalidLength))
    ));
    assert!(matches!(
        prepare_error,
        Err(DspPublicationError::Build(AudioError::InvalidValue {
            field: "samples",
            ..
        }))
    ));
    assert!(processed.is_ok());
    assert_eq!(callback.program().revision(), "pending");
}

#[test]
fn enqueue_reports_rate_full_and_disconnected_without_replacing_active() {
    // Given
    let active_assets = assets(0.1, 4);
    let queued_assets = assets(0.2, 4);
    let full_assets = assets(0.3, 4);
    let wrong_rate_assets = assets(0.4, 4);
    let disconnected_assets = assets(0.5, 4);
    let (mut control, mut callback) = DspSwap::channel(executor("active", &active_assets, 4));

    // When
    let rate = control.enqueue(executor_at_rate("rate", &wrong_rate_assets, 4, 44_100));
    let queued = control.enqueue(executor("queued", &queued_assets, 4));
    let full = control.enqueue(executor("full", &full_assets, 4));
    assert_eq!(callback.program().revision(), "active");
    assert!(callback.process(&mut [[0.0; 2]; 1]).is_ok());
    assert_eq!(callback.program().revision(), "queued");
    drop(callback);
    let disconnected = control.enqueue(executor("disconnected", &disconnected_assets, 4));

    // Then
    assert!(matches!(
        rate,
        Err(DspPublicationError::SampleRateMismatch {
            expected: RATE,
            actual: 44_100
        })
    ));
    assert!(queued.is_ok());
    assert!(matches!(full, Err(DspPublicationError::QueueFull)));
    assert!(matches!(
        disconnected,
        Err(DspPublicationError::Disconnected)
    ));
}

#[test]
fn deferred_retirement_stalls_next_replacement_until_reclaim() {
    // Given
    let a = assets(0.1, 8);
    let b = assets(0.2, 8);
    let c = assets(0.3, 8);
    let d = assets(0.4, 8);
    let (mut control, mut callback) = DspSwap::channel(executor("a", &a, 8));
    let mut output = [[0.0; 2]; 1];
    assert!(control.enqueue(executor("b", &b, 8)).is_ok());
    assert!(callback.process(&mut output).is_ok());
    assert!(control.enqueue(executor("c", &c, 8)).is_ok());
    assert!(callback.process(&mut output).is_ok());
    assert!(control.enqueue(executor("d", &d, 8)).is_ok());

    // When
    assert!(callback.process(&mut output).is_ok());
    let stalled_revision = callback.program().revision().to_owned();
    let reclaimed = control.reclaim();
    assert!(callback.process(&mut output).is_ok());

    // Then
    assert_eq!(stalled_revision, "c");
    assert_eq!(reclaimed, 1);
    assert_eq!(callback.program().revision(), "d");
    assert_eq!(control.reclaim(), 1);
}

#[test]
fn endpoint_drop_shutdown_and_scoped_borrowed_pcm_are_supported() {
    // Given / When / Then: callback-first disconnects publication.
    let first = assets(0.1, 2);
    let next = assets(0.2, 2);
    let (mut control, callback) = DspSwap::channel(executor("first", &first, 2));
    drop(callback);
    assert!(matches!(
        control.enqueue(executor("next", &next, 2)),
        Err(DspPublicationError::Disconnected)
    ));

    // Given / When / Then: control-first still adopts its queued program.
    let (mut control, mut callback) = DspSwap::channel(executor("first", &first, 2));
    assert!(control.enqueue(executor("next", &next, 2)).is_ok());
    drop(control);
    assert!(callback.process(&mut [[0.0; 2]; 1]).is_ok());
    assert_eq!(callback.program().revision(), "next");
    drop(callback);

    // Given / When / Then: explicit shutdown drops both endpoints after quiescence.
    let (control, callback) = DspSwap::channel(executor("first", &first, 2));
    control.shutdown(callback);

    // Given / When / Then: executors may borrow PCM across scoped callback/control threads.
    std::thread::scope(|scope| {
        let (mut control, mut callback) = DspSwap::channel(executor("first", &first, 2));
        let published = std::sync::Arc::new(std::sync::Barrier::new(2));
        let callback_published = std::sync::Arc::clone(&published);
        let callback_thread = scope.spawn(move || {
            callback_published.wait();
            callback.process(&mut [[0.0; 2]; 1])
        });
        assert!(control.enqueue(executor("next", &next, 2)).is_ok());
        published.wait();
        assert!(callback_thread.join().is_ok_and(|result| result.is_ok()));
    });
}

#[test]
fn fresh_replacement_recovers_from_sticky_fault_and_reset_is_active_only() {
    // Given
    let mut fault_assets = assets(f32::MAX, 2);
    let healthy_assets = assets(0.2, 2);
    let pending_assets = assets(0.4, 2);
    fault_assets
        .get_mut("asset")
        .unwrap_or_else(|| panic!("fixture asset missing"))
        .samples[0] = f32::MAX;
    let (mut control, mut callback) = DspSwap::channel(fault_executor(&fault_assets));
    let mut output = [[0.0; 2]; 2];
    let fault = callback.process(&mut output);
    assert!(
        control
            .enqueue(executor("healthy", &healthy_assets, 2))
            .is_ok()
    );
    assert!(callback.process(&mut output[..1]).is_ok());
    assert!(
        control
            .enqueue(executor("pending", &pending_assets, 2))
            .is_ok()
    );

    // When
    callback.reset();
    let active_cursor = callback.cursor();
    let replacement = callback.process(&mut output[..1]);

    // Then
    assert!(matches!(
        fault,
        Err(DspProcessError::NonFiniteTrack { track: 0 })
    ));
    assert_eq!(active_cursor, 0);
    assert!(replacement.is_ok());
    assert_eq!(callback.program().revision(), "pending");
    assert_eq!(callback.cursor(), 1);
}
