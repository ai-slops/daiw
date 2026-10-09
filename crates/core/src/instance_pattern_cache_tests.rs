use crate::definition_composition::{WorkMeter, compose_metered};
use crate::instance_pattern_cache::{InstancePatternCache, PatternRequest};
use crate::resolution::ResolvedOperation;
use crate::{CacheLimits, Note, NotePatch, Operation, Tick, create_project};

fn fixture() -> crate::Project {
    let mut project = create_project(Some("pattern cache unit"), true);
    project.definitions.truncate(1);
    project.instances.truncate(1);
    project.definitions[0].children.clear();
    project.definitions[0].slots.clear();
    project.definitions[0].notes.truncate(1);
    project.instances[0].definition_id = project.definitions[0].id.clone();
    project
}

fn limits() -> CacheLimits {
    CacheLimits {
        max_entries: 4,
        max_retained_bytes: 1_000_000,
        max_notes: 100,
        max_path_entries: 100,
    }
}

fn composed(
    project: &crate::Project,
) -> Result<Vec<crate::definition_composition::ComposedNote>, crate::CoreError> {
    compose_metered(project, &project.definitions[0], &mut WorkMeter::new())
}

fn resolve(
    cache: &mut InstancePatternCache,
    project: &crate::Project,
    composed: &[crate::definition_composition::ComposedNote],
    operations: &[ResolvedOperation],
    work: &mut WorkMeter,
) -> Result<Vec<crate::pattern_resolution::PatternNote>, crate::CoreError> {
    cache.resolve(
        PatternRequest {
            project,
            instance: &project.instances[0],
            definition: &project.definitions[0],
            composed: composed.to_vec(),
            operations,
        },
        work,
    )
}

#[test]
fn zero_work_trace_replays_the_same_already_exceeded_error() -> Result<(), crate::CoreError> {
    // Given
    let project = fixture();
    let composed = composed(&project)?;
    let operations = [ResolvedOperation {
        operation: Operation::Gain { db: 1.0 },
        modifier_id: Some("gain".into()),
    }];
    let mut cache = InstancePatternCache::new(limits());
    resolve(
        &mut cache,
        &project,
        &composed,
        &operations,
        &mut WorkMeter::new(),
    )?;
    let mut warm_meter = exceeded_meter();
    let mut cold_meter = exceeded_meter();

    // When
    let warm = resolve(
        &mut cache,
        &project,
        &composed,
        &operations,
        &mut warm_meter,
    );
    let cold = resolve(
        &mut InstancePatternCache::new(CacheLimits::disabled()),
        &project,
        &composed,
        &operations,
        &mut cold_meter,
    );

    // Then
    assert_eq!(
        warm.err().map(|error| error.to_string()),
        cold.err().map(|error| error.to_string())
    );
    assert_eq!(warm_meter.used(), cold_meter.used());
    Ok(())
}

#[test]
fn multi_operation_trace_fails_at_the_same_chunk() -> Result<(), crate::CoreError> {
    // Given
    let project = fixture();
    let composed = composed(&project)?;
    let operations = [1.0, 2.0].map(|semitones| ResolvedOperation {
        operation: Operation::Transpose { semitones },
        modifier_id: None,
    });
    let mut cache = InstancePatternCache::new(limits());
    resolve(
        &mut cache,
        &project,
        &composed,
        &operations,
        &mut WorkMeter::new(),
    )?;

    // When
    let mut warm_meter = WorkMeter::with_limit(1);
    let warm = resolve(
        &mut cache,
        &project,
        &composed,
        &operations,
        &mut warm_meter,
    );
    let mut cold_meter = WorkMeter::with_limit(1);
    let cold = resolve(
        &mut InstancePatternCache::new(CacheLimits::disabled()),
        &project,
        &composed,
        &operations,
        &mut cold_meter,
    );

    // Then
    assert_eq!(
        warm.err().map(|error| error.to_string()),
        cold.err().map(|error| error.to_string())
    );
    assert_eq!(warm_meter.used(), 2);
    assert_eq!(warm_meter.used(), cold_meter.used());
    Ok(())
}

#[test]
fn failed_transform_records_physical_work_but_is_not_admitted() -> Result<(), crate::CoreError> {
    // Given
    let project = fixture();
    let composed = composed(&project)?;
    let operations = [
        ResolvedOperation {
            operation: Operation::Transpose { semitones: 1.0 },
            modifier_id: None,
        },
        ResolvedOperation {
            operation: Operation::PatchNotes(NotePatch {
                updates: vec![Note {
                    id: "missing".into(),
                    start: Tick::default(),
                    duration: Tick::from_ticks(1),
                    pitch: 60.0,
                    velocity: 0.8,
                }],
                added: Vec::new(),
                removed: Vec::new(),
            }),
            modifier_id: None,
        },
    ];
    let mut cache = InstancePatternCache::new(limits());
    cache.begin_request();

    // When
    let first = resolve(
        &mut cache,
        &project,
        &composed,
        &operations,
        &mut WorkMeter::new(),
    );
    let second = resolve(
        &mut cache,
        &project,
        &composed,
        &operations,
        &mut WorkMeter::new(),
    );
    cache.finish_request();

    // Then
    assert!(first.is_err());
    assert!(second.is_err());
    assert_eq!(cache.stats().resident.entries, 0);
    assert_eq!(cache.stats().last_request.misses, 2);
    assert_eq!(cache.stats().last_request.expanded_work, 6);
    Ok(())
}

#[test]
fn retained_pattern_obeys_byte_bound_and_preserves_useful_entry() -> Result<(), crate::CoreError> {
    // Given
    let project = fixture();
    let composed = composed(&project)?;
    let operations = [ResolvedOperation {
        operation: Operation::Transpose { semitones: 1.0 },
        modifier_id: Some("transpose".into()),
    }];
    let mut cache = InstancePatternCache::new(CacheLimits {
        max_retained_bytes: 1,
        ..limits()
    });

    // When
    let notes = resolve(
        &mut cache,
        &project,
        &composed,
        &operations,
        &mut WorkMeter::new(),
    )?;

    // Then
    assert_eq!(
        notes[0].modifier_ids,
        [crate::ModifierId::from("transpose")]
    );
    assert_eq!(cache.stats().resident.entries, 0);
    assert_eq!(cache.stats().totals.evictions, 0);
    Ok(())
}

fn exceeded_meter() -> WorkMeter {
    let mut meter = WorkMeter::with_limit(0);
    assert!(meter.consume(1).is_err());
    meter
}
