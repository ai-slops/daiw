use crate::composition_cache::{CompositionCache, ResolutionSession};
use crate::composition_cache_metrics::CacheLimits;
use crate::create_project;
use crate::definition_composition::WorkMeter;

fn project() -> crate::Project {
    let mut project = create_project(Some("replay unit"), true);
    project.definitions.truncate(1);
    project.definitions[0].children.clear();
    project.definitions[0].slots.clear();
    project.definitions[0].notes.truncate(1);
    project
}

fn cache() -> CompositionCache {
    CompositionCache::new(CacheLimits {
        max_entries: 2,
        max_retained_bytes: 1_000_000,
        max_notes: 10,
        max_path_entries: 10,
    })
}

#[test]
fn cached_replay_preserves_budget_error() {
    // Given
    let project = project();
    let definition = &project.definitions[0];
    let mut cache = cache();
    let mut session = ResolutionSession::new(&mut cache);
    session
        .compose(&project, definition, &mut WorkMeter::new())
        .unwrap_or_else(|error| panic!("cache warm failed: {error:?}"));

    // When
    let warm = session.compose(&project, definition, &mut WorkMeter::with_limit(0));
    let mut uncached = CompositionCache::new(CacheLimits::disabled());
    let cold = ResolutionSession::new(&mut uncached).compose(
        &project,
        definition,
        &mut WorkMeter::with_limit(0),
    );

    // Then
    assert_eq!(
        warm.err().map(|error| error.to_string()),
        cold.err().map(|error| error.to_string())
    );
}

#[test]
fn zero_work_hit_is_a_noop_after_meter_already_exceeded() {
    // Given
    let mut project = project();
    project.definitions[0].notes.clear();
    let definition = &project.definitions[0];
    let mut cache = cache();
    ResolutionSession::new(&mut cache)
        .compose(&project, definition, &mut WorkMeter::new())
        .unwrap_or_else(|error| panic!("empty cache warm failed: {error:?}"));
    let mut cached_meter = exceeded_meter();
    let mut disabled_meter = exceeded_meter();

    // When
    let cached =
        ResolutionSession::new(&mut cache).compose(&project, definition, &mut cached_meter);
    let mut disabled = CompositionCache::new(CacheLimits::disabled());
    let uncached =
        ResolutionSession::new(&mut disabled).compose(&project, definition, &mut disabled_meter);

    // Then
    assert_eq!(
        cached.as_ref().ok().map(Vec::len),
        uncached.as_ref().ok().map(Vec::len)
    );
    assert_eq!(cached.err().map(|error| error.to_string()), None);
    assert_eq!(cached_meter.used(), disabled_meter.used());
}

#[test]
fn positive_work_hit_advances_once_after_meter_already_exceeded() {
    // Given
    let project = project();
    let definition = &project.definitions[0];
    let mut cache = cache();
    ResolutionSession::new(&mut cache)
        .compose(&project, definition, &mut WorkMeter::new())
        .unwrap_or_else(|error| panic!("nonempty cache warm failed: {error:?}"));
    let mut cached_meter = exceeded_meter();
    let mut disabled_meter = exceeded_meter();

    // When
    let cached =
        ResolutionSession::new(&mut cache).compose(&project, definition, &mut cached_meter);
    let mut disabled = CompositionCache::new(CacheLimits::disabled());
    let uncached =
        ResolutionSession::new(&mut disabled).compose(&project, definition, &mut disabled_meter);

    // Then
    assert_eq!(
        cached.err().map(|error| error.to_string()),
        uncached.err().map(|error| error.to_string())
    );
    assert_eq!(cached_meter.used(), disabled_meter.used());
}

#[test]
fn replay_uses_the_same_checked_overflow_as_single_step_consumption() {
    // Given
    let mut replayed = WorkMeter::with_usage(u64::MAX, u64::MAX);
    let mut stepped = WorkMeter::with_usage(u64::MAX, u64::MAX);

    // When
    let replay_error = replayed
        .consume_replayed(7)
        .err()
        .map(|error| error.to_string());
    let step_error = stepped.consume(1).err().map(|error| error.to_string());

    // Then
    assert_eq!(replay_error, step_error);
    assert_eq!(replayed.used(), stepped.used());
}

fn exceeded_meter() -> WorkMeter {
    let mut meter = WorkMeter::with_limit(0);
    assert!(meter.consume(1).is_err());
    meter
}
