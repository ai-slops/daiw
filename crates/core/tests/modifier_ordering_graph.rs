use daiw_core::{
    ChangeRequest, Command, CoreError, Modifier, ModifierOrderEdge, ModifierOrdering, Operation,
    Principal, Project, apply_change, create_project, snapshot, validate_project,
};

fn edge(before: &str, after: &str) -> ModifierOrderEdge {
    ModifierOrderEdge {
        before: before.into(),
        after: after.into(),
    }
}

fn fixture() -> Project {
    let mut project = create_project(None, true);
    let mut added = project.definitions[0].notes[0].clone();
    added.id = "extra".into();
    added.pitch = 60.0;
    project.modifiers = vec![
        Modifier {
            id: "transpose".into(),
            name: "Transpose".into(),
            operations: vec![Operation::Transpose { semitones: 12.0 }],
        },
        Modifier {
            id: "gain".into(),
            name: "Gain".into(),
            operations: vec![Operation::Gain { db: 0.0 }],
        },
        Modifier {
            id: "add".into(),
            name: "Add".into(),
            operations: vec![Operation::AddNotes { notes: vec![added] }],
        },
    ];
    project.instances[0].modifiers = project
        .modifiers
        .iter()
        .map(|modifier| modifier.id.clone())
        .collect();
    project.instances[0].modifier_ordering = Some(ModifierOrdering {
        edges: vec![edge("add", "gain"), edge("gain", "transpose")],
    });
    project
}

fn change(project: &Project, command: Command) -> Result<Project, CoreError> {
    apply_change(
        project,
        Principal::Human,
        ChangeRequest {
            expected_revision: project.revision.clone(),
            command,
        },
    )
}

#[test]
fn directed_paths_count_but_a_shared_successor_does_not() -> Result<(), CoreError> {
    let mut project = fixture();
    let rendered = snapshot(&project, None)?;
    assert!(
        rendered
            .notes
            .iter()
            .any(|note| note.origin.note_id.as_str() == "extra" && note.pitch == 72.0)
    );
    project.instances[0].modifier_ordering = Some(ModifierOrdering {
        edges: vec![edge("add", "gain"), edge("transpose", "gain")],
    });
    assert!(matches!(
        validate_project(&project),
        Err(CoreError::ModifierOrderRequired { .. })
    ));
    Ok(())
}

#[test]
fn unknown_duplicate_and_self_edges_are_invalid() {
    for edges in [
        vec![edge("add", "unknown")],
        vec![edge("add", "add")],
        vec![edge("add", "gain"), edge("add", "gain")],
    ] {
        let mut project = fixture();
        project.instances[0].modifier_ordering = Some(ModifierOrdering { edges });
        assert!(matches!(
            validate_project(&project),
            Err(CoreError::ModifierOrderingInvalid(_))
        ));
    }
}

#[test]
fn removal_prunes_edges_but_cannot_silently_lose_surviving_order() -> Result<(), CoreError> {
    let project = fixture();
    let remove = Command::RemoveModifier {
        instance_id: project.instances[0].id.clone(),
        modifier_id: "gain".into(),
    };
    assert!(matches!(
        change(&project, remove.clone()),
        Err(CoreError::ModifierOrderRequired { .. })
    ));
    let connected = change(
        &project,
        Command::SetModifierOrdering {
            instance_id: project.instances[0].id.clone(),
            ordering: ModifierOrdering {
                edges: vec![
                    edge("add", "gain"),
                    edge("gain", "transpose"),
                    edge("add", "transpose"),
                ],
            },
        },
    )?;
    let removed = change(&connected, remove)?;
    assert_eq!(
        removed.instances[0].modifier_ordering,
        Some(ModifierOrdering {
            edges: vec![edge("add", "transpose")]
        })
    );
    Ok(())
}

#[test]
fn attachment_can_supply_complete_order_atomically() -> Result<(), CoreError> {
    let mut project = fixture();
    project.instances[0].modifiers = vec!["transpose".into()];
    project.instances[0].modifier_ordering = Some(ModifierOrdering { edges: vec![] });
    let attach = Command::ApplyModifier {
        instance_id: project.instances[0].id.clone(),
        modifier_id: "add".into(),
        ordering: None,
    };
    assert!(matches!(
        change(&project, attach),
        Err(CoreError::ModifierOrderRequired { .. })
    ));
    let attach = Command::ApplyModifier {
        instance_id: project.instances[0].id.clone(),
        modifier_id: "add".into(),
        ordering: Some(ModifierOrdering {
            edges: vec![edge("add", "transpose")],
        }),
    };
    let attached = change(&project, attach.clone())?;
    snapshot(&attached, None)?;
    assert!(matches!(
        change(&attached, attach),
        Err(CoreError::InvalidParams(_))
    ));
    Ok(())
}

#[test]
fn explicit_graph_limits_apply_before_deduplication_or_reachability() {
    let mut edges = fixture();
    edges.instances[0].modifier_ordering = Some(ModifierOrdering {
        edges: vec![edge("add", "gain"); 4097],
    });
    assert!(matches!(
        validate_project(&edges),
        Err(CoreError::ModifierOrderingInvalid(_))
    ));
    let mut nodes = fixture();
    nodes.instances[0].modifiers = vec!["gain".into(); 257];
    assert!(matches!(
        validate_project(&nodes),
        Err(CoreError::ModifierOrderingInvalid(_))
    ));
}

#[test]
fn explicit_empty_survives_duplication_and_last_modifier_removal() -> Result<(), CoreError> {
    let mut project = fixture();
    project.instances[0].modifiers = vec!["gain".into()];
    project.instances[0].modifier_ordering = Some(ModifierOrdering { edges: vec![] });
    let duplicate = change(
        &project,
        Command::DuplicateInstance {
            instance_id: project.instances[0].id.clone(),
        },
    )?;
    assert_eq!(
        duplicate
            .instances
            .last()
            .map(|instance| &instance.modifier_ordering),
        Some(&project.instances[0].modifier_ordering)
    );
    let removed = change(
        &project,
        Command::RemoveModifier {
            instance_id: project.instances[0].id.clone(),
            modifier_id: "gain".into(),
        },
    )?;
    assert_eq!(
        removed.instances[0].modifier_ordering,
        Some(ModifierOrdering { edges: vec![] })
    );
    Ok(())
}
