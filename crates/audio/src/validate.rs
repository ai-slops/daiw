use std::collections::HashSet;

use crate::{AudioError, RenderRequest};

pub fn validate_request(request: &RenderRequest, sample_rate: u32) -> Result<(), AudioError> {
    if !request.snapshot.tempo.is_finite() || request.snapshot.tempo <= 0.0 {
        return Err(AudioError::InvalidTempo);
    }
    if !request.snapshot.length.is_finite() || request.snapshot.length < 0.0 {
        return Err(AudioError::InvalidLength);
    }
    if sample_rate == 0 {
        return Err(AudioError::InvalidSampleRate);
    }
    let track_ids: HashSet<&str> = request
        .snapshot
        .tracks
        .iter()
        .map(|track| track.id.as_str())
        .collect();
    for track in &request.snapshot.tracks {
        finite(&track.id, "gain", track.gain)?;
        finite(&track.id, "pan", track.pan)?;
        positive(&track.id, "cutoff", track.cutoff)?;
    }
    for note in &request.snapshot.notes {
        track_reference(&track_ids, &note.id, &note.track_id)?;
        nonnegative(&note.id, "start", note.start)?;
        nonnegative(&note.id, "duration", note.duration)?;
        finite(&note.id, "pitch", note.pitch)?;
        nonnegative(&note.id, "velocity", note.velocity)?;
    }
    for region in &request.snapshot.regions {
        track_reference(&track_ids, &region.id, &region.track_id)?;
        nonnegative(&region.id, "start", region.start)?;
        positive(&region.id, "duration", region.duration)?;
        if !region.gain.is_finite() || !(-96.0..=24.0).contains(&region.gain) {
            return Err(AudioError::InvalidValue {
                entity_id: region.id.clone(),
                field: "gain",
            });
        }
        nonnegative(&region.id, "sourceOffset", region.source_offset)?;
        if !request.assets.contains_key(&region.asset_id) {
            return Err(AudioError::MissingAsset {
                asset_id: region.asset_id.clone(),
            });
        }
    }
    for (asset_id, asset) in &request.assets {
        if asset.channels == 0 || asset.sample_rate == 0 {
            return Err(AudioError::InvalidAssetChannels {
                asset_id: asset_id.clone(),
                channels: asset.channels,
            });
        }
        if asset.samples.len() % usize::from(asset.channels) != 0 {
            return Err(AudioError::InvalidAssetLayout {
                asset_id: asset_id.clone(),
                channels: asset.channels,
            });
        }
        if asset.samples.iter().any(|sample| !sample.is_finite()) {
            return Err(AudioError::InvalidValue {
                entity_id: asset_id.clone(),
                field: "samples",
            });
        }
    }
    Ok(())
}

fn track_reference(
    tracks: &HashSet<&str>,
    entity_id: &str,
    track_id: &str,
) -> Result<(), AudioError> {
    if tracks.contains(track_id) {
        Ok(())
    } else {
        Err(AudioError::UnknownTrack {
            entity_id: entity_id.to_owned(),
            track_id: track_id.to_owned(),
        })
    }
}

fn finite(entity_id: &str, field: &'static str, value: f64) -> Result<(), AudioError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(AudioError::InvalidValue {
            entity_id: entity_id.to_owned(),
            field,
        })
    }
}

fn positive(entity_id: &str, field: &'static str, value: f64) -> Result<(), AudioError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(AudioError::InvalidValue {
            entity_id: entity_id.to_owned(),
            field,
        })
    }
}

fn nonnegative(entity_id: &str, field: &'static str, value: f64) -> Result<(), AudioError> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(AudioError::InvalidValue {
            entity_id: entity_id.to_owned(),
            field,
        })
    }
}
