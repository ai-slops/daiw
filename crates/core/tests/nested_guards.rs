use daiw_core::{
    Acceptance, ChangeRequest, ChildPlacement, Command, EditScope, Modifier, Note, NoteSelector,
    Operation, Principal, ProposalParams, QueryNotesParams, Tick, accept_proposal, apply_change,
    create_project, create_proposal, query_notes, snapshot,
};

fn beat(value: f64) -> Tick {
    Tick::from_beats(value).unwrap_or_else(|error| panic!("invalid test beat: {error}"))
}

fn note(id: &str, start: f64, pitch: f64) -> Note {
    Note {
        id: id.into(),
        start: beat(start),
        duration: beat(0.5),
        pitch,
        velocity: 0.8,
    }
}

fn project() -> daiw_core::Project {
    let mut project = create_project(Some("Nested guards"), true);
    project.definitions.truncate(2);
    project.instances.truncate(2);
    project.definitions[0].length = beat(4.0);
    project.definitions[0].notes = vec![note("root", 0.0, 60.0)];
    project.definitions[0].slots.clear();
    project.definitions[1].length = beat(1.0);
    project.definitions[1].notes = vec![note("leaf", 0.0, 64.0)];
    project.definitions[0].children = vec![ChildPlacement {
        id: "placement".into(),
        definition_id: project.definitions[1].id.clone(),
        start: beat(1.0),
        length: beat(2.0),
    }];
    for instance in &mut project.instances {
        instance.definition_id = project.definitions[0].id.clone();
        instance.length = beat(4.0);
    }
    project
}

fn apply(project: &daiw_core::Project, command: Command) -> daiw_core::Project {
    apply_change(
        project,
        Principal::Human,
        ChangeRequest {
            expected_revision: project.revision.clone(),
            command,
        },
    )
    .unwrap_or_else(|error| panic!("change failed: {error:?}"))
}

#[test]
fn nested_modifier_binding_stales_on_descendant_change_and_explicit_rebase_refreshes_it() {
    // Given
    let mut project = project();
    project.modifiers.push(Modifier {
        id: "replace".into(),
        name: "Replace".into(),
        operations: vec![Operation::ReplaceSlot {
            slot: "notes".into(),
            notes: vec![note("replacement", 0.0, 72.0)],
        }],
    });
    let instance_id = project.instances[0].id.clone();
    let project = apply(
        &project,
        Command::ApplyModifier {
            instance_id: instance_id.clone(),
            modifier_id: "replace".into(),
            ordering: None,
        },
    );
    let leaf_id = project.definitions[1].id.clone();
    let changed = apply(
        &project,
        Command::EditDefinitionNotes {
            definition_id: leaf_id,
            notes: vec![note("leaf", 0.0, 65.0)],
        },
    );

    // When
    let stale = snapshot(&changed, None);
    let rebased = apply(
        &changed,
        Command::RebaseModifier {
            instance_id,
            modifier_id: "replace".into(),
        },
    );

    // Then
    assert!(stale.is_err());
    assert!(snapshot(&rebased, None).is_ok());
    let binding = &rebased.instances[0].modifier_bindings[&daiw_core::ModifierId::from("replace")];
    assert_eq!(binding.definition_revisions.as_ref().map(Vec::len), Some(2));
    assert_eq!(binding.slots["notes"].cardinality, 3);
}

#[test]
fn nested_modifier_binding_without_revision_closure_is_stale() {
    // Given
    let mut project = project();
    project.modifiers.push(Modifier {
        id: "replace".into(),
        name: "Replace".into(),
        operations: vec![Operation::ReplaceSlot {
            slot: "notes".into(),
            notes: vec![note("replacement", 0.0, 72.0)],
        }],
    });
    let instance_id = project.instances[0].id.clone();
    let mut project = apply(
        &project,
        Command::ApplyModifier {
            instance_id,
            modifier_id: "replace".into(),
            ordering: None,
        },
    );
    project.instances[0]
        .modifier_bindings
        .get_mut(&daiw_core::ModifierId::from("replace"))
        .unwrap_or_else(|| panic!("missing binding"))
        .definition_revisions = None;

    // When
    let rendered = snapshot(&project, None);

    // Then
    assert!(rendered.is_err());
}

#[test]
fn proposal_is_conservatively_stale_after_descendant_revision_changes() {
    // Given
    let mut project = project();
    let proposal = create_proposal(
        &project,
        ProposalParams {
            expected_revision: project.revision.clone(),
            instance_id: project.instances[0].id.clone(),
            request: "explicit".into(),
            operations: Some(vec![Operation::Transpose { semitones: 12.0 }]),
        },
    )
    .unwrap_or_else(|error| panic!("proposal failed: {error:?}"));
    let proposal_id = proposal.id.clone();
    let hunk_id = proposal.hunks[0].id.clone();
    project.proposals.push(proposal);
    let leaf_id = project.definitions[1].id.clone();
    let changed = apply(
        &project,
        Command::EditDefinitionNotes {
            definition_id: leaf_id,
            notes: vec![note("leaf", 0.0, 66.0)],
        },
    );

    // When
    let accepted = accept_proposal(
        &changed,
        Principal::Human,
        Acceptance {
            expected_revision: changed.revision.clone(),
            proposal_id,
            hunk_ids: vec![hunk_id],
        },
    );

    // Then
    assert!(accepted.is_err());
}

#[test]
fn nested_proposal_requires_revision_closure_on_anchor_and_slot() {
    // Given
    let project = project();
    let proposal = create_proposal(
        &project,
        ProposalParams {
            expected_revision: project.revision.clone(),
            instance_id: project.instances[0].id.clone(),
            request: "explicit".into(),
            operations: Some(vec![Operation::ReplaceSlot {
                slot: "notes".into(),
                notes: vec![note("replacement", 0.0, 72.0)],
            }]),
        },
    )
    .unwrap_or_else(|error| panic!("proposal failed: {error:?}"));
    let mut missing_anchor = project.clone();
    let mut anchor_proposal = proposal.clone();
    anchor_proposal
        .base_anchor
        .as_mut()
        .unwrap_or_else(|| panic!("missing anchor"))
        .definition_revisions = None;
    let anchor_hunk = anchor_proposal.hunks[0].id.clone();
    let anchor_id = anchor_proposal.id.clone();
    missing_anchor.proposals.push(anchor_proposal);
    let mut missing_slot = project;
    let mut slot_proposal = proposal;
    slot_proposal.hunks[0]
        .slot_binding
        .as_mut()
        .unwrap_or_else(|| panic!("missing slot binding"))
        .definition_revisions = None;
    let slot_hunk = slot_proposal.hunks[0].id.clone();
    let slot_id = slot_proposal.id.clone();
    missing_slot.proposals.push(slot_proposal);

    // When
    let anchor_result = accept_proposal(
        &missing_anchor,
        Principal::Human,
        Acceptance {
            expected_revision: missing_anchor.revision.clone(),
            proposal_id: anchor_id,
            hunk_ids: vec![anchor_hunk],
        },
    );
    let slot_result = accept_proposal(
        &missing_slot,
        Principal::Human,
        Acceptance {
            expected_revision: missing_slot.revision.clone(),
            proposal_id: slot_id,
            hunk_ids: vec![slot_hunk],
        },
    );

    // Then
    assert!(anchor_result.is_err());
    assert!(slot_result.is_err());
}

#[test]
fn local_nested_pattern_edit_changes_one_occurrence_and_not_a_peer_instance() {
    // Given
    let project = project();
    let target = project.instances[0].id.clone();
    let peer = project.instances[1].id.clone();
    let handle = query_notes(
        &project,
        QueryNotesParams {
            instance_id: target.clone(),
            scope: EditScope::Instance,
            selector: NoteSelector::All,
            max_results: 16,
        },
    )
    .unwrap_or_else(|error| panic!("query failed: {error:?}"));
    let mut editable = vec![
        note(handle.note_ids[0].as_str(), 0.0, 60.0),
        note(handle.note_ids[1].as_str(), 1.0, 64.0),
        note(handle.note_ids[2].as_str(), 2.0, 64.0),
    ];
    editable[1].pitch = 80.0;

    // When
    let changed = apply(
        &project,
        Command::EditNotes {
            instance_id: target.clone(),
            scope: EditScope::Instance,
            notes: editable,
        },
    );
    let rendered = snapshot(&changed, None)
        .unwrap_or_else(|error| panic!("changed snapshot failed: {error:?}"));

    // Then
    assert_eq!(
        rendered
            .notes
            .iter()
            .filter(|note| note.instance_id == target && note.pitch == 80.0)
            .count(),
        1
    );
    assert_eq!(
        rendered
            .notes
            .iter()
            .filter(|note| note.instance_id == peer && note.pitch == 80.0)
            .count(),
        0
    );
}
