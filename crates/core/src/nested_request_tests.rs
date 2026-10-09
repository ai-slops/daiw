use crate::definition_composition::{WorkMeter, compose_metered};
use crate::note_resolution::{NoteRequest, resolve_composed};
use crate::{Tick, create_project};

#[test]
fn cropped_repetition_counts_only_emitted_events() {
    // Given
    let mut project = create_project(Some("Exact event count"), true);
    project.definitions[0].length = Tick::from_ticks(10);
    project.definitions[0].slots.clear();
    project.definitions[0].children.clear();
    project.definitions[0].notes.truncate(1);
    project.definitions[0].notes[0].start = Tick::from_ticks(9);
    project.definitions[0].notes[0].duration = Tick::from_ticks(1);
    project.instances[0].definition_id = project.definitions[0].id.clone();
    project.instances[0].length = Tick::from_ticks(11);
    let definition = &project.definitions[0];
    let instance = &project.instances[0];

    // When
    let mut meter = WorkMeter::new();
    let composed = compose_metered(&project, definition, &mut meter)
        .unwrap_or_else(|error| panic!("composition failed: {error:?}"));
    let notes = resolve_composed(
        NoteRequest {
            definition,
            instance,
            track_id: &instance.track_id,
            operations: &[],
            event_budget: 1,
        },
        composed,
        &mut meter,
    )
    .unwrap_or_else(|error| panic!("exact event budget failed: {error:?}"));

    // Then
    assert_eq!(notes.len(), 1);
}

#[test]
fn composition_meter_is_shared_across_calls() {
    // Given
    let mut project = create_project(Some("Shared meter"), true);
    for definition in &mut project.definitions[..2] {
        definition.notes.truncate(1);
        definition.children.clear();
    }
    let mut meter = WorkMeter::with_limit(1);

    // When
    let first = compose_metered(&project, &project.definitions[0], &mut meter);
    let second = compose_metered(&project, &project.definitions[1], &mut meter);

    // Then
    assert!(first.is_ok());
    assert!(second.is_err());
}
