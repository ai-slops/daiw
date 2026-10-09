use crate::ids::RetainedCapacity;
use crate::pattern_resolution::PatternNote;
use std::mem::size_of;

pub(crate) fn path_entries(notes: &[PatternNote]) -> Option<usize> {
    notes.iter().try_fold(0_usize, |total, item| {
        total.checked_add(item.definition_path.as_ref().map_or(0, Vec::len))
    })
}

pub(crate) fn retained_payload_bytes(
    notes: &[PatternNote],
    note_capacity: usize,
    trace_capacity: usize,
) -> Option<usize> {
    payload_bytes(notes, note_capacity, trace_capacity, true)
}

pub(crate) fn minimum_payload_bytes(notes: &[PatternNote], trace_length: usize) -> Option<usize> {
    payload_bytes(notes, notes.len(), trace_length, false)
}

fn payload_bytes(
    notes: &[PatternNote],
    note_capacity: usize,
    trace_capacity: usize,
    retained_capacity: bool,
) -> Option<usize> {
    let notes_inline = note_capacity.checked_mul(size_of::<PatternNote>())?;
    let trace = trace_capacity.checked_mul(size_of::<usize>())?;
    notes
        .iter()
        .try_fold(notes_inline.checked_add(trace)?, |total, item| {
            let paths = item.definition_path.as_ref().map_or(Some(0), |path| {
                let count = if retained_capacity {
                    path.capacity()
                } else {
                    path.len()
                };
                let inline = count.checked_mul(size_of::<crate::music::DefinitionPathEntry>())?;
                path.iter().try_fold(inline, |sum, entry| {
                    sum.checked_add(string_bytes(
                        entry.definition_id.as_str(),
                        entry.definition_id.retained_capacity(),
                        retained_capacity,
                    ))?
                    .checked_add(
                        entry.placement_id.as_ref().map_or(0, |id| {
                            string_bytes(id.as_str(), id.retained_capacity(), retained_capacity)
                        }),
                    )
                })
            })?;
            let modifier_count = if retained_capacity {
                item.modifier_ids.capacity()
            } else {
                item.modifier_ids.len()
            };
            let modifiers = item.modifier_ids.iter().try_fold(
                modifier_count.checked_mul(size_of::<crate::ids::ModifierId>())?,
                |sum, id| {
                    sum.checked_add(string_bytes(
                        id.as_str(),
                        id.retained_capacity(),
                        retained_capacity,
                    ))
                },
            )?;
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
                .checked_add(paths)?
                .checked_add(modifiers)
        })
}

const fn string_bytes(value: &str, capacity: usize, retained_capacity: bool) -> usize {
    if retained_capacity {
        capacity
    } else {
        value.len()
    }
}
