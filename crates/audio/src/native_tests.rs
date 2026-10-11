use cpal::{SampleFormat, SampleRate, SupportedBufferSize, SupportedStreamConfigRange};

use super::{NativeRenderFormat, StreamErrors, select_output_config};
use crate::native_device::{
    NativeDeviceSelector, NativeOutputDevice, default_name_flags, device_name_unique_flags,
    output_device_name_sha256, selector_matches_descriptors,
};

#[derive(Clone, Copy)]
struct TestConfig {
    channels: u16,
    minimum_rate: u32,
    maximum_rate: u32,
    sample_format: SampleFormat,
}

fn config(fixture: TestConfig) -> SupportedStreamConfigRange {
    SupportedStreamConfigRange::new(
        fixture.channels,
        SampleRate(fixture.minimum_rate),
        SampleRate(fixture.maximum_rate),
        SupportedBufferSize::Range { min: 64, max: 2048 },
        fixture.sample_format,
    )
}

#[test]
fn stream_errors_are_counted_without_formatting_or_logging() {
    // Given
    let errors = StreamErrors::default();

    // When
    errors.record();
    errors.record();

    // Then
    assert_eq!(errors.count(), 2);
}

#[test]
fn exact_render_format_is_selected_when_default_like_formats_are_incompatible()
-> Result<(), &'static str> {
    // Given
    let supported = [
        config(TestConfig {
            channels: 2,
            minimum_rate: 44_100,
            maximum_rate: 44_100,
            sample_format: SampleFormat::F32,
        }),
        config(TestConfig {
            channels: 6,
            minimum_rate: 48_000,
            maximum_rate: 48_000,
            sample_format: SampleFormat::F32,
        }),
        config(TestConfig {
            channels: 2,
            minimum_rate: 48_000,
            maximum_rate: 48_000,
            sample_format: SampleFormat::I16,
        }),
        config(TestConfig {
            channels: 2,
            minimum_rate: 48_000,
            maximum_rate: 96_000,
            sample_format: SampleFormat::F32,
        }),
    ];

    // When
    let selected = select_output_config(
        supported,
        NativeRenderFormat {
            sample_rate: 48_000,
            channels: 2,
        },
    );

    // Then
    let selected = selected.ok_or("exact f32 stereo 48 kHz config must be selected")?;
    assert_eq!(selected.sample_format(), SampleFormat::F32);
    assert_eq!(selected.channels(), 2);
    assert_eq!(selected.sample_rate(), SampleRate(48_000));
    Ok(())
}

#[test]
fn no_config_is_selected_when_only_sample_type_differs() {
    // Given
    let supported = [config(TestConfig {
        channels: 2,
        minimum_rate: 48_000,
        maximum_rate: 48_000,
        sample_format: SampleFormat::I16,
    })];

    // When
    let selected = select_output_config(
        supported,
        NativeRenderFormat {
            sample_rate: 48_000,
            channels: 2,
        },
    );

    // Then
    assert!(selected.is_none());
}

#[test]
fn device_name_digest_is_stable_sha256() {
    // Given / When / Then
    assert_eq!(
        output_device_name_sha256("DAIW output"),
        "77dde207fce4b2749ea74fa3b721f0f5ffddd212dfc712bf2cc01d5c36e11d29"
    );
}

#[test]
fn duplicate_default_names_are_not_claimed_as_default() {
    // Given
    let names = ["same", "other", "same"];

    // When
    let flags = default_name_flags(&names, Some("same"));

    // Then
    assert_eq!(flags, [false, false, false]);
    assert_eq!(
        default_name_flags(&names, Some("other")),
        [false, true, false]
    );
}

#[test]
fn duplicate_device_names_are_unsupported_and_unselectable() {
    // Given
    let digest = output_device_name_sha256("duplicate");
    let selector = NativeDeviceSelector::new(0, &digest).map_err(|error| error.to_string());
    let descriptors = [
        NativeOutputDevice {
            index: 0,
            name: "duplicate".into(),
            name_sha256: digest.clone(),
            is_default: false,
            supported: true,
        },
        NativeOutputDevice {
            index: 1,
            name: "duplicate".into(),
            name_sha256: digest,
            is_default: false,
            supported: true,
        },
    ];

    // When
    let unique = device_name_unique_flags(&["duplicate", "duplicate"]);
    let selectable = selector.map(|value| selector_matches_descriptors(&value, &descriptors));

    // Then
    assert_eq!(unique, [false, false]);
    assert_eq!(selectable, Ok(false));
}
