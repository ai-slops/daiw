use crate::composition_cache::{CompositionCache, ResolutionSession};
use crate::composition_cache_metrics::CacheLimits;
use crate::definition_composition::WorkMeter;
use crate::{Tick, create_project};

fn project() -> crate::Project {
    let mut project = create_project(Some("cache unit"), true);
    project.definitions.truncate(2);
    for definition in &mut project.definitions {
        definition.children.clear();
        definition.slots.clear();
        definition.notes.truncate(1);
    }
    project
}

fn limits() -> CacheLimits {
    CacheLimits {
        max_entries: 2,
        max_retained_bytes: 1_000_000,
        max_notes: 10,
        max_path_entries: 10,
    }
}

#[test]
fn failed_composition_is_not_inserted() {
    // Given
    let project = project();
    let definition = &project.definitions[0];
    let mut cache = CompositionCache::new(limits());
    cache.begin_request();

    // When
    let mut session = ResolutionSession::new(&mut cache);
    let failed = session.compose(&project, definition, &mut WorkMeter::with_limit(0));
    let succeeded = session.compose(&project, definition, &mut WorkMeter::new());
    cache.finish_request();

    // Then
    assert!(failed.is_err());
    assert!(succeeded.is_ok());
    assert_eq!(cache.stats().resident.entries, 1);
    assert_eq!(cache.stats().totals.misses, 2);
    assert_eq!(cache.stats().totals.expanded_work, 2);
}

#[test]
fn oversized_entry_does_not_evict_useful_entry() {
    // Given
    let mut project = project();
    project.definitions[1].notes = (0..3)
        .map(|index| crate::Note {
            id: format!("large-{index}").into(),
            start: Tick::from_ticks(index),
            duration: Tick::from_ticks(1),
            pitch: 60.0,
            velocity: 0.8,
        })
        .collect();
    let limits = CacheLimits {
        max_notes: 1,
        ..limits()
    };
    let mut cache = CompositionCache::new(limits);
    cache.begin_request();

    // When
    let mut session = ResolutionSession::new(&mut cache);
    session
        .compose(&project, &project.definitions[0], &mut WorkMeter::new())
        .unwrap_or_else(|error| panic!("small composition failed: {error:?}"));
    session
        .compose(&project, &project.definitions[1], &mut WorkMeter::new())
        .unwrap_or_else(|error| panic!("large composition failed: {error:?}"));
    let _ = session.compose(&project, &project.definitions[0], &mut WorkMeter::new());
    cache.finish_request();

    // Then
    assert_eq!(cache.stats().resident.entries, 1);
    assert_eq!(cache.stats().totals.hits, 1);
    assert_eq!(cache.stats().totals.evictions, 0);
}

#[test]
fn least_recently_used_entry_is_evicted_deterministically() {
    // Given
    let mut project = project();
    let mut third = project.definitions[1].clone();
    third.id = "third".into();
    project.definitions.push(third);
    let mut cache = CompositionCache::new(limits());
    cache.begin_request();

    // When
    let mut session = ResolutionSession::new(&mut cache);
    for index in [0, 1, 0, 2, 1] {
        let _ = session.compose(&project, &project.definitions[index], &mut WorkMeter::new());
    }
    cache.finish_request();

    // Then
    assert_eq!(cache.stats().resident.entries, 2);
    assert_eq!(cache.stats().totals.hits, 1);
    assert_eq!(cache.stats().totals.misses, 4);
    assert_eq!(cache.stats().totals.evictions, 2);
}

#[test]
fn byte_and_path_bounds_skip_individually_oversized_entries() {
    // Given
    let mut project = project();
    project.definitions[0].notes.clear();
    project.definitions[0].length = Tick::from_ticks(2);
    project.definitions[1].length = Tick::from_ticks(1);
    project.definitions[0].children = vec![crate::ChildPlacement {
        id: "nested".into(),
        definition_id: project.definitions[1].id.clone(),
        start: Tick::default(),
        length: Tick::from_ticks(2),
    }];
    let byte_limits = CacheLimits {
        max_retained_bytes: 1,
        ..limits()
    };
    let path_limits = CacheLimits {
        max_path_entries: 1,
        ..limits()
    };

    // When
    let residents = [byte_limits, path_limits].map(|limits| {
        let mut cache = CompositionCache::new(limits);
        cache.begin_request();
        let _ = ResolutionSession::new(&mut cache).compose(
            &project,
            &project.definitions[0],
            &mut WorkMeter::new(),
        );
        cache.finish_request();
        cache.stats().resident
    });

    // Then
    assert!(residents.iter().all(|resident| resident.entries == 0));
}

#[test]
fn aggregate_byte_note_and_path_limits_evict_lru_entries() {
    // Given
    let mut nested = project();
    nested.definitions[0].notes.clear();
    nested.definitions[0].length = Tick::from_ticks(1);
    nested.definitions[1].length = Tick::from_ticks(1);
    nested.definitions[0].children = vec![crate::ChildPlacement {
        id: "nested".into(),
        definition_id: nested.definitions[1].id.clone(),
        start: Tick::default(),
        length: Tick::from_ticks(1),
    }];
    let mut projects = [nested.clone(), nested.clone(), nested];
    for (index, project) in projects.iter_mut().enumerate() {
        project.id = format!("aggregate-{index}").into();
    }
    let mut probe = CompositionCache::new(limits());
    compose_root(&mut probe, &projects[0]);
    let one_entry_bytes = probe.stats().resident.retained_bytes;
    let bounded = [
        CacheLimits {
            max_retained_bytes: one_entry_bytes,
            ..limits()
        },
        CacheLimits {
            max_notes: 1,
            ..limits()
        },
        CacheLimits {
            max_path_entries: 2,
            ..limits()
        },
    ];

    // When
    let stats = bounded.map(|limits| {
        let mut cache = CompositionCache::new(limits);
        cache.begin_request();
        for project in &projects {
            compose_root(&mut cache, project);
        }
        cache.finish_request();
        cache.stats()
    });

    // Then
    assert!(
        stats.iter().all(|stats| stats.resident.entries == 1),
        "{stats:?}"
    );
    assert!(stats.iter().all(|stats| stats.totals.evictions == 2));
    assert!(stats.iter().all(|stats| {
        stats.resident.retained_bytes <= stats.limits.max_retained_bytes
            && stats.resident.notes <= stats.limits.max_notes
            && stats.resident.path_entries <= stats.limits.max_path_entries
    }));
}

fn compose_root(cache: &mut CompositionCache, project: &crate::Project) {
    ResolutionSession::new(cache)
        .compose(project, &project.definitions[0], &mut WorkMeter::new())
        .unwrap_or_else(|error| panic!("aggregate composition failed: {error:?}"));
}

#[test]
fn larger_entry_is_admitted_after_evicting_and_compacting_small_entries() {
    // Given two small entries and a byte budget exactly fitting one larger entry.
    let small = project();
    let mut second = small.clone();
    second.id = "second-project".into();
    let mut large = small.clone();
    large.definitions[0].notes[0].id = "large-note".repeat(400).into();
    let mut probe = CompositionCache::new(limits());
    compose_root(&mut probe, &large);
    let max_retained_bytes = probe.stats().resident.retained_bytes;
    let mut cache = CompositionCache::new(CacheLimits {
        max_retained_bytes,
        ..limits()
    });
    compose_root(&mut cache, &small);
    compose_root(&mut cache, &second);
    assert_eq!(cache.stats().resident.entries, 2);

    // When the larger entry replaces both smaller entries.
    cache.begin_request();
    compose_root(&mut cache, &large);
    compose_root(&mut cache, &large);
    cache.finish_request();

    // Then it remains cached, fits the bound, and the second request is a hit.
    let stats = cache.stats();
    assert_eq!(stats.resident.entries, 1);
    assert_eq!(stats.resident.retained_bytes, max_retained_bytes);
    assert_eq!(stats.last_request.evictions, 2);
    assert_eq!(stats.last_request.hits, 1);
}
