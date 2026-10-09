use crate::definition_composition::{WorkMeter, compose_metered};
use crate::ids::RetainedCapacity;
use crate::instance_pattern_cache::{InstancePatternCache, PatternRequest};
use crate::resolution::ResolvedOperation;
use crate::{CacheLimits, Note, Operation, Tick, create_project};

#[test]
fn owned_composition_moves_provenance_capacity_into_pattern() -> Result<(), crate::CoreError> {
    // Given
    let mut project = create_project(Some("owned pattern"), true);
    project.definitions.truncate(1);
    project.instances.truncate(1);
    project.definitions[0].children.clear();
    let mut composed = compose_metered(&project, &project.definitions[0], &mut WorkMeter::new())?;
    let mut id = String::with_capacity(128);
    id.push_str("capacity-proof");
    composed[0].note.id = id.into();
    let retained_capacity = composed[0].note.id.retained_capacity();

    // When
    let pattern = crate::pattern_resolution::from_composed(
        composed,
        &project.definitions[0],
        &[],
        &mut WorkMeter::new(),
        &mut Vec::new(),
    )?;

    // Then
    assert_eq!(pattern[0].note.id.retained_capacity(), retained_capacity);
    Ok(())
}

#[test]
fn oversized_pattern_does_not_evict_useful_entry() -> Result<(), crate::CoreError> {
    // Given
    let mut project = create_project(Some("pattern admission"), true);
    project.definitions.truncate(1);
    project.instances.truncate(1);
    project.definitions[0].children.clear();
    project.definitions[0].notes.truncate(1);
    let composed = compose_metered(&project, &project.definitions[0], &mut WorkMeter::new())?;
    let small = [ResolvedOperation {
        operation: Operation::Transpose { semitones: 1.0 },
        modifier_id: None,
    }];
    let oversized = [ResolvedOperation {
        operation: Operation::AddNotes {
            notes: vec![Note {
                id: "added".into(),
                start: Tick::default(),
                duration: Tick::from_ticks(1),
                pitch: 60.0,
                velocity: 0.8,
            }],
        },
        modifier_id: None,
    }];
    let mut cache = InstancePatternCache::new(CacheLimits {
        max_entries: 2,
        max_retained_bytes: 1_000_000,
        max_notes: 1,
        max_path_entries: 100,
    });
    cache.begin_request();

    // When
    for operations in [&small[..], &oversized[..], &small[..]] {
        cache.resolve(
            PatternRequest {
                project: &project,
                instance: &project.instances[0],
                definition: &project.definitions[0],
                composed: composed.clone(),
                operations,
            },
            &mut WorkMeter::new(),
        )?;
    }
    cache.finish_request();

    // Then
    let stats = cache.stats();
    assert_eq!(stats.resident.entries, 1);
    assert_eq!(stats.last_request.hits, 1);
    assert_eq!(stats.last_request.misses, 2);
    assert_eq!(stats.last_request.evictions, 0);
    assert!(stats.resident.notes <= stats.limits.max_notes);
    assert!(stats.resident.retained_bytes <= stats.limits.max_retained_bytes);
    Ok(())
}
