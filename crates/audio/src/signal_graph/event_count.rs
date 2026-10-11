use crate::render_clock::RenderClock;
use crate::source::SourceMetadataProvider;
use crate::{AudioError, Snapshot};

use super::events::{prepare_note, prepare_region};

pub(super) fn count_prepared_events(
    snapshot: &Snapshot,
    sources: &impl SourceMetadataProvider,
    clock: &RenderClock,
    soloed: bool,
) -> Result<usize, AudioError> {
    let mut count = 0_usize;
    for track in snapshot
        .tracks
        .iter()
        .filter(|track| !track.mute && (!soloed || track.solo))
    {
        for note in snapshot
            .notes
            .iter()
            .filter(|note| note.track_id == track.id)
        {
            count = count
                .checked_add(usize::from(
                    prepare_note(note, track.instrument, clock)?.is_some(),
                ))
                .ok_or(AudioError::InvalidFrameCount)?;
        }
        for region in snapshot
            .regions
            .iter()
            .filter(|region| region.track_id == track.id)
        {
            count = count
                .checked_add(usize::from(
                    prepare_region(region, 0, sources, clock)?.is_some(),
                ))
                .ok_or(AudioError::InvalidFrameCount)?;
        }
    }
    Ok(count)
}
