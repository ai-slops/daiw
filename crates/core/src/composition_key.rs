use crate::error::CoreError;
use crate::music::{ChildPlacement, Definition, Note};
use crate::project::Project;
use std::collections::{BTreeMap, BTreeSet};

const KEY_DOMAIN: &[u8] = b"daiw:source-composition-cache:v1\0";

pub(crate) fn composition_key(project: &Project, root: &Definition) -> Result<[u8; 32], CoreError> {
    let mut definitions = BTreeMap::new();
    for definition in &project.definitions {
        if definitions.insert(&definition.id, definition).is_some() {
            return Err(CoreError::InvalidProject(format!(
                "duplicate definition id {}",
                definition.id
            )));
        }
    }
    let mut reachable = BTreeSet::from([&root.id]);
    let mut pending = vec![&root.id];
    while let Some(id) = pending.pop() {
        let definition = definitions
            .get(id)
            .ok_or_else(|| CoreError::NotFound(id.to_string()))?;
        for child in &definition.children {
            if reachable.insert(&child.definition_id) {
                pending.push(&child.definition_id);
            }
        }
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(KEY_DOMAIN);
    hash_str(&mut hasher, project.id.as_str());
    hash_str(&mut hasher, root.id.as_str());
    for id in reachable {
        let definition = definitions
            .get(id)
            .ok_or_else(|| CoreError::NotFound(id.to_string()))?;
        hash_definition(&mut hasher, definition);
    }
    Ok(*hasher.finalize().as_bytes())
}

fn hash_definition(hasher: &mut blake3::Hasher, definition: &Definition) {
    hash_str(hasher, definition.id.as_str());
    hasher.update(&definition.revision.to_le_bytes());
    hasher.update(&definition.length.ticks().to_le_bytes());
    hasher.update(&definition.notes.len().to_le_bytes());
    for note in &definition.notes {
        hash_note(hasher, note);
    }
    let mut children: Vec<_> = definition.children.iter().collect();
    children.sort_by(|left, right| left.id.cmp(&right.id));
    hasher.update(&children.len().to_le_bytes());
    for child in children {
        hash_child(hasher, child);
    }
}

fn hash_note(hasher: &mut blake3::Hasher, note: &Note) {
    hash_str(hasher, note.id.as_str());
    hasher.update(&note.start.ticks().to_le_bytes());
    hasher.update(&note.duration.ticks().to_le_bytes());
    hasher.update(&note.pitch.to_bits().to_le_bytes());
    hasher.update(&note.velocity.to_bits().to_le_bytes());
}

fn hash_child(hasher: &mut blake3::Hasher, child: &ChildPlacement) {
    hash_str(hasher, child.id.as_str());
    hash_str(hasher, child.definition_id.as_str());
    hasher.update(&child.start.ticks().to_le_bytes());
    hasher.update(&child.length.ticks().to_le_bytes());
}

fn hash_str(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(&value.len().to_le_bytes());
    hasher.update(value.as_bytes());
}
