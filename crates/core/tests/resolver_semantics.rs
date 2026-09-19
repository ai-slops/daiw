use daiw_core::{
    Modifier, Operation, Parameter, ProposalParams, ProposalStatus, create_project,
    create_proposal, snapshot, validate_project,
};
use std::collections::BTreeSet;
use std::fmt::Debug;

fn success<T, E: Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("unexpected error: {error:?}"),
    }
}

#[test]
fn instance_gain_uses_a_derived_track_without_changing_peers() {
    // Given
    let mut project = create_project(Some("Effective tracks"), true);
    let changed_instance = project.instances[0].id.clone();
    let peer_instance = project.instances[1].id.clone();
    let base_track = project.instances[0].track_id.clone();
    project.instances[0]
        .overrides
        .push(Operation::Gain { db: 3.0 });

    // When
    let rendered = success(snapshot(&project, None));
    let changed_track = rendered
        .notes
        .iter()
        .find(|note| note.instance_id == changed_instance)
        .map(|note| note.track_id.clone());
    let peer_track = rendered
        .notes
        .iter()
        .find(|note| note.instance_id == peer_instance)
        .map(|note| note.track_id.clone());

    // Then
    assert_ne!(changed_track.as_ref(), Some(&base_track));
    assert_eq!(peer_track.as_ref(), Some(&base_track));
    let derived = rendered
        .tracks
        .iter()
        .find(|track| Some(&track.id) == changed_track.as_ref());
    assert_eq!(derived.map(|track| track.gain), Some(3.0));
    assert_eq!(
        rendered
            .tracks
            .iter()
            .find(|track| track.id == base_track)
            .map(|track| track.gain),
        Some(0.0)
    );
}

#[test]
fn proposal_preview_applies_only_pending_hunks() {
    // Given
    let mut project = create_project(Some("Preview"), true);
    let instance_id = project.instances[0].id.clone();
    let mut proposal = success(create_proposal(
        &project,
        ProposalParams {
            expected_revision: project.revision.clone(),
            instance_id: instance_id.clone(),
            request: "explicit".into(),
            operations: Some(vec![
                Operation::Transpose { semitones: 12.0 },
                Operation::Velocity { factor: 0.8 },
            ]),
        },
    ));
    proposal.hunks[0].accepted = true;
    proposal.status = ProposalStatus::PartiallyAccepted;
    project.instances[0]
        .overrides
        .push(proposal.hunks[0].operation.clone());
    let proposal_id = proposal.id.clone();
    project.proposals.push(proposal);

    // When
    let rendered = success(snapshot(&project, Some(&proposal_id)));
    let root = rendered.notes.iter().find(|note| {
        note.instance_id == instance_id && note.origin.note_id.as_str() == "note-chorus-root"
    });

    // Then
    assert_eq!(root.map(|note| note.pitch), Some(73.0));
    assert_eq!(root.map(|note| note.velocity), Some(0.86 * 0.8));
}

#[test]
fn rejected_proposal_cannot_be_previewed() {
    // Given
    let mut project = create_project(Some("Rejected preview"), true);
    let mut proposal = success(create_proposal(
        &project,
        ProposalParams {
            expected_revision: project.revision.clone(),
            instance_id: project.instances[0].id.clone(),
            request: "octave up".into(),
            operations: None,
        },
    ));
    proposal.status = ProposalStatus::Rejected;
    let proposal_id = proposal.id.clone();
    project.proposals.push(proposal);

    // When
    let error = match snapshot(&project, Some(&proposal_id)) {
        Ok(_) => panic!("rejected proposal previewed"),
        Err(error) => error,
    };

    // Then
    assert_eq!(error.code(), "ProposalClosed");
}

#[test]
fn long_instance_repeats_definition_with_unique_cropped_events() {
    // Given
    let project = create_project(Some("Repeat"), true);
    let instance = &project.instances[3];

    // When
    let first: Vec<_> = success(snapshot(&project, None))
        .notes
        .into_iter()
        .filter(|note| note.instance_id == instance.id)
        .collect();
    let second_ids: Vec<_> = success(snapshot(&project, None))
        .notes
        .into_iter()
        .filter(|note| note.instance_id == instance.id)
        .map(|note| note.id)
        .collect();

    // Then
    assert_eq!(first.len(), 64);
    assert_eq!(
        first
            .iter()
            .map(|note| &note.id)
            .collect::<BTreeSet<_>>()
            .len(),
        64
    );
    assert!(
        first
            .iter()
            .all(|note| note.start + note.duration <= instance.start + instance.length)
    );
    assert_eq!(
        first.iter().map(|note| note.id.clone()).collect::<Vec<_>>(),
        second_ids
    );
}

#[test]
fn validation_rejects_conflicts_across_modifier_chain() {
    // Given
    let mut project = create_project(Some("Combined conflict"), true);
    for (id, value) in [("pan-a", -0.2), ("pan-b", 0.2)] {
        project.modifiers.push(Modifier {
            id: id.into(),
            name: id.into(),
            operations: vec![Operation::SetParameter {
                parameter: Parameter::Pan,
                value,
            }],
        });
        project.instances[0].modifiers.push(id.into());
    }

    // When
    let error = match validate_project(&project) {
        Ok(()) => panic!("combined conflict accepted"),
        Err(error) => error,
    };

    // Then
    assert_eq!(error.code(), "ModifierConflict");
}
