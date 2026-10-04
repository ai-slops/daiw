use daiw_core::{
    CoreError, Modifier, ModifierOrderEdge, ModifierOrdering, Note, Operation, Parameter, Project,
    Tick, create_project, snapshot, validate_project,
};

fn candidate(operations: Vec<Vec<Operation>>, ordered: bool) -> Project {
    let mut project = create_project(None, true);
    project.modifiers = operations
        .into_iter()
        .enumerate()
        .map(|(index, operations)| Modifier {
            id: format!("m{index}").into(),
            name: "Test".into(),
            operations,
        })
        .collect();
    project.instances[0].modifiers = project
        .modifiers
        .iter()
        .map(|modifier| modifier.id.clone())
        .collect();
    project.instances[0].modifier_ordering = Some(ModifierOrdering {
        edges: if ordered {
            vec![ModifierOrderEdge {
                before: "m0".into(),
                after: "m1".into(),
            }]
        } else {
            vec![]
        },
    });
    project
}

fn note(id: &str) -> Note {
    Note {
        id: id.into(),
        start: Tick::default(),
        duration: Tick::from_ticks(960),
        pitch: 60.0,
        velocity: 0.8,
    }
}

#[test]
fn velocity_clamping_requires_order_and_obeys_declared_direction() -> Result<(), CoreError> {
    let operations = vec![
        vec![Operation::Velocity { factor: 2.0 }],
        vec![Operation::Velocity { factor: 0.5 }],
    ];
    assert!(matches!(
        validate_project(&candidate(operations.clone(), false)),
        Err(CoreError::ModifierOrderRequired { .. })
    ));
    let project = candidate(operations, true);
    let rendered = snapshot(&project, None)?;
    assert!(
        rendered
            .notes
            .iter()
            .filter(|note| note.instance_id == project.instances[0].id)
            .all(|note| note.velocity == 0.5)
    );
    Ok(())
}

#[test]
fn disjoint_note_fields_can_resolve_without_an_edge() -> Result<(), CoreError> {
    let mut project = candidate(
        vec![
            vec![Operation::Transpose { semitones: 12.0 }],
            vec![Operation::Velocity { factor: 0.5 }],
        ],
        false,
    );
    let expected = snapshot(&project, None)?;
    project.instances[0].modifiers.reverse();
    assert_eq!(snapshot(&project, None)?, expected);
    Ok(())
}

#[test]
fn gain_deltas_are_domain_compatible_without_declared_order() -> Result<(), CoreError> {
    let project = candidate(
        vec![
            vec![Operation::Gain { db: 1.0 }],
            vec![Operation::Gain { db: 2.0 }],
        ],
        false,
    );
    let rendered = snapshot(&project, None)?;
    assert!(rendered.tracks.iter().any(|track| track.id.as_str()
        == "effective:track-lead:instance-chorus-a"
        && track.gain == 3.0));
    Ok(())
}

#[test]
fn disjoint_additions_commute_but_duplicate_added_ids_are_hard_conflicts() -> Result<(), CoreError>
{
    let distinct = candidate(
        vec![
            vec![Operation::AddNotes {
                notes: vec![note("a")],
            }],
            vec![Operation::AddNotes {
                notes: vec![note("b")],
            }],
        ],
        false,
    );
    validate_project(&distinct)?;
    let duplicate = candidate(
        vec![
            vec![Operation::AddNotes {
                notes: vec![note("a")],
            }],
            vec![Operation::AddNotes {
                notes: vec![note("a")],
            }],
        ],
        true,
    );
    assert!(matches!(
        validate_project(&duplicate),
        Err(CoreError::ModifierConflict(_))
    ));
    Ok(())
}

#[test]
fn mixed_modifiers_cannot_hide_note_hazards_behind_gain_compatibility() {
    let project = candidate(
        vec![
            vec![
                Operation::Gain { db: 1.0 },
                Operation::AddNotes {
                    notes: vec![note("added")],
                },
            ],
            vec![
                Operation::Gain { db: 2.0 },
                Operation::Transpose { semitones: 12.0 },
            ],
        ],
        false,
    );
    assert!(matches!(
        validate_project(&project),
        Err(CoreError::ModifierOrderRequired { .. })
    ));
}

#[test]
fn duplicate_setters_and_slots_remain_conflicts_even_with_explicit_order() {
    let setters = candidate(
        vec![
            vec![Operation::SetParameter {
                parameter: Parameter::Pan,
                value: 0.2,
            }],
            vec![Operation::SetParameter {
                parameter: Parameter::Pan,
                value: 0.4,
            }],
        ],
        true,
    );
    let slots = candidate(
        vec![
            vec![Operation::ReplaceSlot {
                slot: "notes".into(),
                notes: vec![],
            }],
            vec![Operation::ReplaceSlot {
                slot: "notes".into(),
                notes: vec![],
            }],
        ],
        true,
    );
    assert!(matches!(
        validate_project(&setters),
        Err(CoreError::ModifierConflict(_))
    ));
    assert!(matches!(
        validate_project(&slots),
        Err(CoreError::ModifierConflict(_))
    ));
}

#[test]
fn duplicate_additions_inside_one_modifier_are_hard_conflicts() {
    let mut project = candidate(
        vec![vec![
            Operation::AddNotes {
                notes: vec![note("duplicate")],
            },
            Operation::AddNotes {
                notes: vec![note("duplicate")],
            },
        ]],
        false,
    );
    assert!(matches!(
        validate_project(&project),
        Err(CoreError::ModifierConflict(_))
    ));
    project.instances[0].modifier_ordering = None;
    assert!(validate_project(&project).is_ok());
}

#[test]
fn duplicate_addition_in_local_override_precedes_missing_order_error() {
    let mut project = candidate(
        vec![
            vec![Operation::AddNotes {
                notes: vec![note("duplicate")],
            }],
            vec![Operation::Transpose { semitones: 12.0 }],
        ],
        false,
    );
    project.instances[0].overrides.push(Operation::AddNotes {
        notes: vec![note("duplicate")],
    });
    assert!(matches!(
        validate_project(&project),
        Err(CoreError::ModifierConflict(_))
    ));
}
