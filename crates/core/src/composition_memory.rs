use crate::definition_composition::ComposedNote;
use crate::ids::RetainedCapacity;
use std::mem::size_of;

pub(crate) fn path_entries(notes: &[ComposedNote]) -> Option<usize> {
    notes.iter().try_fold(0_usize, |total, item| {
        total.checked_add(item.definition_path.as_ref().map_or(0, Vec::len))
    })
}

pub(crate) fn minimum_payload_bytes(notes: &[ComposedNote]) -> Option<usize> {
    payload_bytes(notes, notes.len(), false)
}

pub(crate) fn retained_payload_bytes(notes: &[ComposedNote], capacity: usize) -> Option<usize> {
    payload_bytes(notes, capacity, true)
}

fn payload_bytes(
    notes: &[ComposedNote],
    capacity: usize,
    retained_capacity: bool,
) -> Option<usize> {
    let inline = capacity.checked_mul(size_of::<ComposedNote>())?;
    notes.iter().try_fold(inline, |total, item| {
        let path = item.definition_path.as_ref().map_or(Some(0), |path| {
            let count = if retained_capacity {
                path.capacity()
            } else {
                path.len()
            };
            let inline = count.checked_mul(size_of::<crate::music::DefinitionPathEntry>())?;
            path.iter().try_fold(inline, |path_total, entry| {
                let definition = string_bytes(
                    entry.definition_id.as_str(),
                    entry.definition_id.retained_capacity(),
                    retained_capacity,
                );
                let placement = entry.placement_id.as_ref().map_or(0, |id| {
                    string_bytes(id.as_str(), id.retained_capacity(), retained_capacity)
                });
                path_total.checked_add(definition)?.checked_add(placement)
            })
        })?;
        total
            .checked_add(string_bytes(
                item.note.id.as_str(),
                item.note.id.retained_capacity(),
                retained_capacity,
            ))?
            .checked_add(string_bytes(
                item.definition_id.as_str(),
                item.definition_id.retained_capacity(),
                retained_capacity,
            ))?
            .checked_add(string_bytes(
                item.source_note_id.as_str(),
                item.source_note_id.retained_capacity(),
                retained_capacity,
            ))?
            .checked_add(path)
    })
}

const fn string_bytes(value: &str, capacity: usize, retained_capacity: bool) -> usize {
    if retained_capacity {
        capacity
    } else {
        value.len()
    }
}
