use crate::error::CoreError;
use crate::music::{Note, Operation};
pub use crate::operation_validation::validate_operations;
use crate::operation_validation::validate_reusable_operations;
use crate::project::Project;
use crate::time::MAX_TICKS;
use std::collections::BTreeSet;
use std::fmt::Display;

pub(crate) const MIN_GAIN_DB: f64 = -96.0;
pub(crate) const MAX_GAIN_DB: f64 = 24.0;

pub fn validate_project(project: &Project) -> Result<(), CoreError> {
    if !(20.0..=400.0).contains(&project.tempo) {
        return Err(CoreError::InvalidProject(
            "tempo must be between 20 and 400".to_owned(),
        ));
    }
    if project.meter[0] == 0 || project.meter[1] == 0 {
        return Err(CoreError::InvalidProject(
            "meter values must be positive".to_owned(),
        ));
    }
    let definition_ids = unique_ids(
        project.definitions.iter().map(|item| &item.id),
        "definition",
    )?;
    let track_ids = unique_ids(project.tracks.iter().map(|item| &item.id), "track")?;
    let _instance_ids = unique_ids(project.instances.iter().map(|item| &item.id), "instance")?;
    let modifier_ids = unique_ids(project.modifiers.iter().map(|item| &item.id), "modifier")?;
    for definition in &project.definitions {
        validate_notes(&definition.notes)?;
        if definition.length.ticks() <= 0 || definition.length.ticks() > MAX_TICKS {
            return Err(CoreError::InvalidProject(format!(
                "definition {} has non-positive length",
                definition.id
            )));
        }
        if definition
            .notes
            .iter()
            .any(|note| note.start.ticks() + note.duration.ticks() > definition.length.ticks())
        {
            return Err(CoreError::InvalidProject(format!(
                "definition {} contains notes outside its length",
                definition.id
            )));
        }
        for (name, slot) in &definition.slots {
            if slot.start.ticks() < 0 || slot.end <= slot.start || slot.end > definition.length {
                return Err(CoreError::InvalidProject(format!(
                    "definition {} slot {name} is out of range",
                    definition.id
                )));
            }
        }
    }
    for track in &project.tracks {
        if !(-1.0..=1.0).contains(&track.pan)
            || !track.cutoff.is_finite()
            || track.cutoff <= 0.0
            || !(MIN_GAIN_DB..=MAX_GAIN_DB).contains(&track.gain)
        {
            return Err(CoreError::InvalidProject(format!(
                "track {} parameters are out of range",
                track.id
            )));
        }
    }
    for instance in &project.instances {
        crate::binding_validation::validate(instance)?;
        if !definition_ids.contains(&instance.definition_id)
            || !track_ids.contains(&instance.track_id)
        {
            return Err(CoreError::InvalidProject(format!(
                "instance {} references a missing entity",
                instance.id
            )));
        }
        if instance
            .modifiers
            .iter()
            .any(|id| !modifier_ids.contains(id))
        {
            return Err(CoreError::InvalidProject(format!(
                "instance {} references a missing modifier",
                instance.id
            )));
        }
        if instance.start.ticks() < 0
            || instance.length.ticks() <= 0
            || instance.start.ticks() > MAX_TICKS
            || instance.length.ticks() > MAX_TICKS
            || instance.start.ticks() + instance.length.ticks() > MAX_TICKS
        {
            return Err(CoreError::InvalidProject(format!(
                "instance {} timing is out of range",
                instance.id
            )));
        }
        let mut combined = Vec::new();
        for modifier in crate::modifier_plan::ordered(project, instance)? {
            combined.extend(modifier.operations.iter().cloned());
        }
        combined.extend(instance.overrides.iter().cloned());
        validate_operations(&combined)?;
        let definition = project
            .definitions
            .iter()
            .find(|definition| definition.id == instance.definition_id)
            .ok_or_else(|| CoreError::NotFound(instance.definition_id.to_string()))?;
        for operation in &instance.overrides {
            if let Operation::PatchNotes(patch) = operation {
                crate::note_patch::validate_bounds(&patch.updates, definition.length)?;
                crate::note_patch::validate_bounds(&patch.added, definition.length)?;
            }
        }
    }
    for modifier in &project.modifiers {
        validate_reusable_operations(&modifier.operations)?;
    }
    for proposal in &project.proposals {
        for hunk in &proposal.hunks {
            validate_reusable_operations(std::slice::from_ref(&hunk.operation))?;
        }
    }
    validate_media(project, &track_ids)?;
    Ok(())
}

fn validate_media(
    project: &Project,
    track_ids: &BTreeSet<&crate::ids::TrackId>,
) -> Result<(), CoreError> {
    let mut asset_ids = BTreeSet::new();
    for asset in &project.assets {
        if !asset_ids.insert(&asset.id)
            || asset.frames == 0
            || asset.sample_rate == 0
            || asset.channels == 0
            || asset.hash.trim().is_empty()
        {
            return Err(CoreError::InvalidProject(format!(
                "asset {} is invalid",
                asset.id
            )));
        }
    }
    let mut region_ids = BTreeSet::new();
    for region in &project.regions {
        if !region_ids.insert(&region.id)
            || !track_ids.contains(&region.track_id)
            || !asset_ids.contains(&region.asset_id)
            || region.start.ticks() < 0
            || region.duration.ticks() <= 0
            || region.start.ticks() > MAX_TICKS
            || region.duration.ticks() > MAX_TICKS
            || region.start.ticks() + region.duration.ticks() > MAX_TICKS
            || !(MIN_GAIN_DB..=MAX_GAIN_DB).contains(&region.gain)
            || !region.source_offset.is_finite()
            || region.source_offset < 0.0
        {
            return Err(CoreError::InvalidProject(format!(
                "audio region {} is invalid",
                region.id
            )));
        }
    }
    Ok(())
}

pub fn validate_notes(notes: &[Note]) -> Result<(), CoreError> {
    let mut ids = BTreeSet::new();
    for note in notes {
        if !ids.insert(&note.id) {
            return Err(CoreError::InvalidProject(format!(
                "duplicate note id {}",
                note.id
            )));
        }
        if note.start.ticks() < 0
            || note.duration.ticks() <= 0
            || note.start.ticks() > MAX_TICKS
            || note.duration.ticks() > MAX_TICKS
            || !(0.0..=1.0).contains(&note.velocity)
        {
            return Err(CoreError::InvalidProject(format!(
                "note {} values are out of range",
                note.id
            )));
        }
        if !note.pitch.is_finite() {
            return Err(CoreError::InvalidProject(format!(
                "note {} pitch must be finite",
                note.id
            )));
        }
    }
    Ok(())
}

fn unique_ids<'a, T: Display + Ord + 'a>(
    ids: impl Iterator<Item = &'a T>,
    kind: &str,
) -> Result<BTreeSet<&'a T>, CoreError> {
    let mut unique = BTreeSet::new();
    for id in ids {
        if !unique.insert(id) {
            return Err(CoreError::InvalidProject(format!(
                "duplicate {kind} id {id}"
            )));
        }
    }
    Ok(unique)
}
