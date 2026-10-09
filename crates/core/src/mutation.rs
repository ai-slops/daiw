use crate::command::Command;
use crate::error::CoreError;
use crate::ids::{DefinitionId, InstanceId, ModifierId, TrackId};
use crate::instance_mutation::find_instance_mut;
use crate::music::{Definition, Instance, Modifier, Track};
use crate::project::Project;
use crate::validation::{validate_notes, validate_operations};

pub(crate) fn apply_command_with_session(
    project: &mut Project,
    command: Command,
    session: &mut crate::composition_cache::ResolutionSession<'_>,
) -> Result<String, CoreError> {
    match command {
        Command::SetTempo { tempo } => crate::timing_mutation::set_tempo(project, tempo),
        Command::SetTempoMap { tempo_map } => {
            crate::timing_mutation::set_tempo_map(project, tempo_map)
        }
        Command::SetMeter { meter } => crate::timing_mutation::set_meter(project, meter),
        Command::SetMeterMap { meter_map } => {
            crate::timing_mutation::set_meter_map(project, meter_map)
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
                children: Vec::new(),
            });
            Ok("Added definition".to_owned())
        }
        Command::AddDefinitionChild {
            parent_definition_id,
            definition_id,
            start,
            length,
        } => crate::definition_mutation::add_child(
            project,
            parent_definition_id,
            crate::music::ChildPlacement {
                id: crate::ids::ChildPlacementId::new_random(),
                definition_id,
                start,
                length,
            },
        ),
        Command::SetDefinitionChild {
            parent_definition_id,
            child,
        } => crate::definition_mutation::set_child(project, parent_definition_id, child),
        Command::RemoveDefinitionChild {
            parent_definition_id,
            child_id,
        } => crate::definition_mutation::remove_child(project, parent_definition_id, child_id),
        Command::EditDefinitionNotes {
            definition_id,
            notes,
        } => crate::definition_mutation::edit_notes(project, definition_id, notes),
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
            crate::mutation_helpers::duplicate_instance(project, &instance_id)
        }
        Command::DeleteInstance { instance_id } => {
            crate::mutation_helpers::delete_instance(project, &instance_id)
        }
        Command::EditNotes {
            instance_id,
            scope,
            notes,
        } => crate::instance_mutation::edit_notes_with_session(
            project,
            instance_id,
            scope,
            notes,
            session,
        ),
        Command::TransformSelectedNotes {
            selection,
            transform,
        } => crate::selected_note_edit::apply_with_session(project, selection, transform, session),
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
            let message = crate::instance_mutation::apply_modifier_with_session(
                project,
                instance_id.clone(),
                modifier_id,
                session,
            )?;
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
        } => crate::instance_mutation::rebase_modifier_with_session(
            project,
            instance_id,
            modifier_id,
            session,
        ),
        Command::RemoveModifier {
            instance_id,
            modifier_id,
        } => crate::mutation_helpers::remove_modifier(project, &instance_id, &modifier_id),
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
            crate::mutation_helpers::add_audio_region(project, asset, region)
        }
        Command::RemoveAudioRegion { region_id } => {
            crate::mutation_helpers::remove_audio_region(project, &region_id)
        }
    }
}
