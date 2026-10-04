use crate::error::CoreError;
use crate::ids::NoteId;
use crate::music::{Definition, Note};
use crate::note_selection::MAX_SELECTION_RESULTS;
use crate::time::Tick;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "type"
)]
pub enum NoteSelector {
    All,
    StableIds { note_ids: Vec<NoteId> },
    NamedSlot { slot: String },
    BeatRange { start: Tick, end: Tick },
    PitchRange { min: f64, max: f64 },
}

pub fn normalize(selector: NoteSelector) -> Result<NoteSelector, CoreError> {
    match &selector {
        NoteSelector::All => {}
        NoteSelector::StableIds { note_ids } => {
            let unique = note_ids.iter().collect::<BTreeSet<_>>();
            if note_ids.is_empty()
                || note_ids.len() > MAX_SELECTION_RESULTS
                || unique.len() != note_ids.len()
            {
                return Err(CoreError::InvalidParams(
                    "stableIds must contain 1..4096 unique note IDs".into(),
                ));
            }
        }
        NoteSelector::NamedSlot { slot } => {
            if slot.trim().is_empty() || slot.len() > 256 {
                return Err(CoreError::InvalidParams(
                    "slot must contain 1..256 bytes".into(),
                ));
            }
        }
        NoteSelector::BeatRange { start, end } => {
            if *start < Tick::default() || start >= end {
                return Err(CoreError::InvalidParams(
                    "beat range must be non-negative and increasing".into(),
                ));
            }
        }
        NoteSelector::PitchRange { min, max } => {
            if !min.is_finite() || !max.is_finite() || min > max {
                return Err(CoreError::InvalidParams(
                    "pitch range must be finite and ordered".into(),
                ));
            }
            return Ok(NoteSelector::PitchRange {
                min: if *min == 0.0 { 0.0 } else { *min },
                max: if *max == 0.0 { 0.0 } else { *max },
            });
        }
    }
    Ok(selector)
}

pub fn resolve(
    notes: &[Note],
    definition: &Definition,
    selector: &NoteSelector,
) -> Result<Vec<NoteId>, CoreError> {
    let requested = match selector {
        NoteSelector::StableIds { note_ids } => Some(note_ids.iter().collect::<BTreeSet<_>>()),
        NoteSelector::All
        | NoteSelector::NamedSlot { .. }
        | NoteSelector::BeatRange { .. }
        | NoteSelector::PitchRange { .. } => None,
    };
    let range = match selector {
        NoteSelector::NamedSlot { slot } => Some(
            crate::modifier_binding::select_slot(definition, slot)
                .ok_or_else(|| {
                    CoreError::SelectionInvalid(format!("named slot {slot} is missing"))
                })?
                .range,
        ),
        NoteSelector::All
        | NoteSelector::StableIds { .. }
        | NoteSelector::BeatRange { .. }
        | NoteSelector::PitchRange { .. } => None,
    };
    let mut selected: Vec<_> = notes
        .iter()
        .filter(|note| match selector {
            NoteSelector::All => true,
            NoteSelector::StableIds { .. } => {
                requested.as_ref().is_some_and(|ids| ids.contains(&note.id))
            }
            NoteSelector::NamedSlot { .. } => range
                .as_ref()
                .is_some_and(|range| note.start >= range.start && note.start < range.end),
            NoteSelector::BeatRange { start, end } => note.start >= *start && note.start < *end,
            NoteSelector::PitchRange { min, max } => note.pitch >= *min && note.pitch <= *max,
        })
        .collect();
    if requested
        .as_ref()
        .is_some_and(|ids| ids.len() != selected.len())
    {
        return Err(CoreError::SelectionInvalid(
            "one or more stable note IDs are missing".into(),
        ));
    }
    selected.sort_by(|left, right| {
        left.start
            .cmp(&right.start)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(selected.into_iter().map(|note| note.id.clone()).collect())
}
