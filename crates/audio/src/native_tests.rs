use cpal::{SampleFormat, SampleRate, SupportedBufferSize, SupportedStreamConfigRange};

use super::{NativeRenderFormat, StreamErrors, select_output_config};

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
