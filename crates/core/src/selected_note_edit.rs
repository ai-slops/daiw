use crate::command::EditScope;
use crate::error::CoreError;
use crate::music::Note;
use crate::note_selection::SelectionHandle;
use crate::project::Project;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "type"
)]
pub enum NoteTransform {
    Transpose { semitones: f64 },
    SetVelocity { velocity: f64 },
}

pub(crate) fn apply_with_session(
    project: &mut Project,
    selection: SelectionHandle,
    transform: NoteTransform,
    session: &mut crate::composition_cache::ResolutionSession<'_>,
) -> Result<String, CoreError> {
    validate_transform(&transform)?;
    let mut notes =
        crate::note_selection::validate_handle_with_session(project, &selection, session)?;
    let selected = selection.note_ids.iter().collect::<BTreeSet<_>>();
    let mut changed = false;
    for note in &mut notes {
        if selected.contains(&note.id) {
            let previous = (note.pitch, note.velocity);
            transform_note(note, &transform);
            changed |= previous != (note.pitch, note.velocity);
        }
    }
    if !changed {
        return Err(CoreError::InvalidParams(
            "selected transform does not change any notes".into(),
        ));
    }
    crate::instance_mutation::edit_notes_with_session(
        project,
        selection.instance_id,
        selection.scope,
        notes,
        session,
    )?;
    Ok(match selection.scope {
        EditScope::Source => "Transformed selected source notes".into(),
        EditScope::Instance => "Transformed selected instance notes".into(),
    })
}

fn validate_transform(transform: &NoteTransform) -> Result<(), CoreError> {
    match transform {
        NoteTransform::Transpose { semitones } if semitones.is_finite() => Ok(()),
        NoteTransform::SetVelocity { velocity }
            if velocity.is_finite() && (0.0..=1.0).contains(velocity) =>
        {
            Ok(())
        }
        NoteTransform::Transpose { .. } => Err(CoreError::InvalidParams(
            "transpose semitones must be finite".into(),
        )),
        NoteTransform::SetVelocity { .. } => Err(CoreError::InvalidParams(
            "velocity must be finite and within 0..1".into(),
        )),
    }
}

fn transform_note(note: &mut Note, transform: &NoteTransform) {
    match transform {
        NoteTransform::Transpose { semitones } => note.pitch += semitones,
        NoteTransform::SetVelocity { velocity } => note.velocity = *velocity,
    }
}
