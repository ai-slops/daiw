use crate::command::Command;
use crate::error::CoreError;
use crate::ids::{DefinitionId, InstanceId, ModifierId, TrackId};
use crate::instance_mutation::{apply_modifier, edit_notes, find_instance_mut, rebase_modifier};
use crate::music::{Definition, Instance, Modifier, Track};
use crate::project::Project;
use crate::validation::{validate_notes, validate_operations};

pub fn apply_command(project: &mut Project, command: Command) -> Result<String, CoreError> {
    match command {
        Command::SetTempo { tempo } => {
            project.tempo = tempo;
            Ok("Set tempo".to_owned())
        }
        Command::RenameProject { name } => {
            project.name = name;
            Ok("Renamed project".to_owned())
        }
        Command::AddTrack {
            name,
            instrument,
            color,
            role,
        } => {
            project.tracks.push(Track {
                id: TrackId::new_random(),
                name,
                role,
                color,
                instrument,
                gain: 0.0,
                pan: 0.0,
                mute: false,
                solo: false,
                cutoff: 12_000.0,
            });
            Ok("Added track".to_owned())
        }
        Command::SetTrack {
            track_id,
            gain,
            pan,
            mute,
            solo,
            cutoff,
            name,
        } => {
            let track = project
                .tracks
                .iter_mut()
                .find(|track| track.id == track_id)
                .ok_or_else(|| CoreError::NotFound(track_id.to_string()))?;
            if let Some(value) = gain {
                track.gain = value;
            }
            if let Some(value) = pan {
                track.pan = value;
            }
            if let Some(value) = mute {
                track.mute = value;
            }
            if let Some(value) = solo {
                track.solo = value;
            }
            if let Some(value) = cutoff {
                track.cutoff = value;
            }
            if let Some(value) = name {
                track.name = value;
            }
            Ok("Updated track".to_owned())
        }
        Command::AddDefinition {
            name,
            length,
            notes,
        } => {
            validate_notes(&notes)?;
            project.definitions.push(Definition {
                id: DefinitionId::new_random(),
                name,
                length,
                notes,
                revision: 0,
                slots: std::collections::BTreeMap::new(),
            });
            Ok("Added definition".to_owned())
        }
        Command::Instantiate {
            definition_id,
            track_id,
            start,
            name,
        } => {
            let definition = project
                .definitions
                .iter()
                .find(|item| item.id == definition_id)
                .ok_or_else(|| CoreError::NotFound(definition_id.to_string()))?;
            if !project.tracks.iter().any(|track| track.id == track_id) {
                return Err(CoreError::NotFound(track_id.to_string()));
            }
            project.instances.push(Instance {
                id: InstanceId::new_random(),
                name: name.unwrap_or_else(|| definition.name.clone()),
                definition_id,
                track_id,
                start,
                length: definition.length,
                modifiers: Vec::new(),
                overrides: Vec::new(),
                modifier_bindings: std::collections::BTreeMap::new(),
                modifier_ordering: None,
            });
            Ok("Instantiated definition".to_owned())
        }
        Command::MoveInstance {
            instance_id,
            start,
            track_id,
        } => {
            let instance = find_instance_mut(project, &instance_id)?;
            instance.start = start;
            if let Some(id) = track_id {
                instance.track_id = id;
            }
            Ok("Moved instance".to_owned())
        }
        Command::DuplicateInstance { instance_id } => {
            let mut duplicate = project
                .instances
                .iter()
                .find(|instance| instance.id == instance_id)
                .cloned()
                .ok_or_else(|| CoreError::NotFound(instance_id.to_string()))?;
            duplicate.id = InstanceId::new_random();
            duplicate.name = format!("{} Copy", duplicate.name);
            duplicate.start = duplicate.start + duplicate.length;
            project.instances.push(duplicate);
            Ok("Duplicated instance".to_owned())
        }
        Command::DeleteInstance { instance_id } => {
            let position = project
                .instances
                .iter()
                .position(|instance| instance.id == instance_id)
                .ok_or_else(|| CoreError::NotFound(instance_id.to_string()))?;
            project.instances.remove(position);
            Ok("Deleted instance".to_owned())
        }
        Command::EditNotes {
            instance_id,
            scope,
            notes,
        } => edit_notes(project, instance_id, scope, notes),
        Command::TransformSelectedNotes {
            selection,
            transform,
        } => crate::selected_note_edit::apply(project, selection, transform),
        Command::ApplyModifier {
            instance_id,
            modifier_id,
            ordering,
        } => {
            if ordering.is_some()
                && find_instance_mut(project, &instance_id)?
                    .modifiers
                    .contains(&modifier_id)
            {
                return Err(CoreError::InvalidParams(
                    "use setModifierOrdering for an existing attachment".into(),
                ));
            }
            let message = apply_modifier(project, instance_id.clone(), modifier_id)?;
            if let Some(mut ordering) = ordering {
                ordering.edges.sort();
                find_instance_mut(project, &instance_id)?.modifier_ordering = Some(ordering);
            }
            Ok(message)
        }
        Command::SetModifierOrdering {
            instance_id,
            mut ordering,
        } => {
            ordering.edges.sort();
            find_instance_mut(project, &instance_id)?.modifier_ordering = Some(ordering);
            Ok("Set modifier dependency order".into())
        }
        Command::RebaseModifier {
            instance_id,
            modifier_id,
        } => rebase_modifier(project, instance_id, modifier_id),
        Command::RemoveModifier {
            instance_id,
            modifier_id,
        } => {
            let instance = find_instance_mut(project, &instance_id)?;
            instance.modifiers.retain(|id| *id != modifier_id);
            instance.modifier_bindings.remove(&modifier_id);
            if let Some(ordering) = &mut instance.modifier_ordering {
                ordering
                    .edges
                    .retain(|edge| edge.before != modifier_id && edge.after != modifier_id);
            }
            Ok("Removed modifier".to_owned())
        }
        Command::CreateModifier { name, operations } => {
            validate_operations(&operations)?;
            project.modifiers.push(Modifier {
                id: ModifierId::new_random(),
                name,
                operations,
            });
            Ok("Created modifier".to_owned())
        }
        Command::AddAudioRegion { asset, region } => {
            if let Some(existing) = project
                .assets
                .iter()
                .find(|existing| existing.id == asset.id)
            {
                if existing != &asset {
                    return Err(CoreError::InvalidProject(format!(
                        "asset {} metadata conflicts with its content address",
                        asset.id
                    )));
                }
            } else {
                project.assets.push(asset);
            }
            project.regions.push(region);
            Ok("Added audio region".to_owned())
        }
        Command::RemoveAudioRegion { region_id } => {
            let position = project
                .regions
                .iter()
                .position(|region| region.id == region_id)
                .ok_or_else(|| CoreError::NotFound(region_id.to_string()))?;
            project.regions.remove(position);
            Ok("Removed audio region".to_owned())
        }
    }
}
