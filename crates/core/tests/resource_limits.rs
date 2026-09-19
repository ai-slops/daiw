use daiw_core::{MAX_BEATS, Modifier, Tick, create_project, snapshot, validate_project};

#[test]
fn wire_tick_rejects_values_outside_timeline_policy() {
    // Given
    let encoded = format!("{}", MAX_BEATS + 1.0);

    // When
    let parsed = serde_json::from_str::<Tick>(&encoded);

    // Then
    assert!(parsed.is_err());
}

#[test]
fn snapshot_rejects_effective_event_expansion_over_budget() {
    // Given
    let mut project = create_project(Some("Expansion"), true);
    project.instances[3].length = match Tick::from_beats(MAX_BEATS) {
        Ok(value) => value,
        Err(error) => panic!("valid policy boundary rejected: {error}"),
    };

    // When
    let error = match snapshot(&project, None) {
        Ok(_) => panic!("hostile expansion rendered"),
        Err(error) => error,
    };

    // Then
    assert_eq!(error.code(), "InvalidProject");
    assert!(error.to_string().contains("effective event budget"));
}

#[test]
fn snapshot_budget_is_global_across_instances() {
    // Given
    let mut project = create_project(Some("Global expansion"), true);
    project.definitions[1].notes.truncate(1);
    project.definitions[1].length = match Tick::from_beats(1.0) {
        Ok(value) => value,
        Err(error) => panic!("valid definition length rejected: {error}"),
    };
    project.instances[3].length = match Tick::from_beats(60_000.0) {
        Ok(value) => value,
        Err(error) => panic!("valid instance length rejected: {error}"),
    };
    let mut second = project.instances[3].clone();
    second.id = "second-long-instance".into();
    project.instances.push(second);

    // When
    let error = match snapshot(&project, None) {
        Ok(_) => panic!("aggregate hostile expansion rendered"),
        Err(error) => error,
    };

    // Then
    assert_eq!(error.code(), "InvalidProject");
    assert!(error.to_string().contains("effective event budget"));
}

#[test]
fn validation_rejects_duplicate_domain_ids() {
    // Given
    let fixture = create_project(Some("Duplicates"), true);
    let mut duplicate_definition = fixture.clone();
    duplicate_definition
        .definitions
        .push(fixture.definitions[0].clone());
    let mut duplicate_track = fixture.clone();
    duplicate_track.tracks.push(fixture.tracks[0].clone());
    let mut duplicate_instance = fixture.clone();
    duplicate_instance
        .instances
        .push(fixture.instances[0].clone());
    let mut duplicate_modifier = fixture;
    let modifier = Modifier {
        id: "duplicate".into(),
        name: "Duplicate".into(),
        operations: Vec::new(),
    };
    duplicate_modifier
        .modifiers
        .extend([modifier.clone(), modifier]);

    // When
    let projects = [
        duplicate_definition,
        duplicate_track,
        duplicate_instance,
        duplicate_modifier,
    ];

    // Then
    for project in projects {
        let error = match validate_project(&project) {
            Ok(()) => panic!("duplicate id accepted"),
            Err(error) => error,
        };
        assert_eq!(error.code(), "InvalidProject");
        assert!(error.to_string().contains("duplicate"));
    }
}
