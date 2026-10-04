use crate::{AudioError, Snapshot};
use daiw_timing::{TempoMap, Tick};

pub const MAX_RENDER_FRAMES: usize = 8_640_000;

pub(crate) struct RenderClock {
    pub sample_rate: u32,
    pub tempo: f64,
    tempo_map: Option<TempoMap>,
    pub total_frames: usize,
}

impl RenderClock {
    pub fn new(snapshot: &Snapshot, sample_rate: u32) -> Result<Self, AudioError> {
        let mut clock = Self {
            sample_rate,
            tempo: snapshot.tempo,
            tempo_map: snapshot.tempo_map.clone(),
            total_frames: 0,
        };
        clock.total_frames = clock.frame_at(snapshot.length)?;
        if clock.total_frames > MAX_RENDER_FRAMES {
            return Err(AudioError::RenderTooLarge {
                limit_frames: MAX_RENDER_FRAMES,
            });
        }
        Ok(clock)
    }

    pub fn frame_at(&self, beat: f64) -> Result<usize, AudioError> {
        let seconds = match &self.tempo_map {
            Some(map) => map
                .seconds_at(
                    Tick::from_beats(beat.max(0.0)).map_err(|_| AudioError::InvalidFrameCount)?,
                )
                .map_err(|_| AudioError::InvalidFrameCount)?,
            None => beat.max(0.0) * 60.0 / self.tempo,
        };
        checked_frame_index((seconds * f64::from(self.sample_rate)).round())
    }

    pub fn frames_for(&self, start: f64, duration: f64) -> Result<usize, AudioError> {
        let Some(map) = &self.tempo_map else {
            return self.frame_at(duration);
        };
        let start_tick = Tick::from_beats(start).map_err(|_| AudioError::InvalidFrameCount)?;
        let duration_tick =
            Tick::from_beats(duration).map_err(|_| AudioError::InvalidFrameCount)?;
        let start_seconds = map
            .seconds_at(start_tick)
            .map_err(|_| AudioError::InvalidFrameCount)?;
        let end_seconds = map
            .seconds_at(start_tick + duration_tick)
            .map_err(|_| AudioError::InvalidFrameCount)?;
        let start_frame =
            checked_frame_index((start_seconds * f64::from(self.sample_rate)).round())?;
        let end_frame = checked_frame_index((end_seconds * f64::from(self.sample_rate)).round())?;
        end_frame
            .checked_sub(start_frame)
            .ok_or(AudioError::InvalidFrameCount)
    }
}

pub(crate) fn checked_frame_index(value: f64) -> Result<usize, AudioError> {
    if !value.is_finite() || value < 0.0 || value >= usize::MAX as f64 {
        return Err(AudioError::InvalidFrameCount);
    }
    // The range check prevents float-to-integer saturation; callers retain their rounding policy.
    Ok(value as usize)
}

pub(crate) fn silent_buffer(samples: usize) -> Result<Vec<f32>, AudioError> {
    let mut buffer = Vec::new();
    buffer.try_reserve_exact(samples)?;
    buffer.resize(samples, 0.0);
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::{MAX_RENDER_FRAMES, RenderClock, silent_buffer};
    use crate::{AudioError, Snapshot};

    #[test]
    fn frame_budget_accepts_exact_limit_without_allocating_audio() -> Result<(), AudioError> {
        let mut snapshot = Snapshot {
            revision: "limit".into(),
            tempo: 60.0,
            tempo_map: None,
            length: 180.0,
            tracks: vec![],
            notes: vec![],
            regions: vec![],
        };
        assert_eq!(
            RenderClock::new(&snapshot, 48_000)?.total_frames,
            MAX_RENDER_FRAMES
        );
        snapshot.length += 1.0 / 48_000.0;
        assert!(matches!(
            RenderClock::new(&snapshot, 48_000),
            Err(AudioError::RenderTooLarge { .. })
        ));
        snapshot.length = f64::MAX;
        assert!(matches!(
            RenderClock::new(&snapshot, 48_000),
            Err(AudioError::InvalidFrameCount)
        ));
        Ok(())
    }

    #[test]
    fn impossible_reservation_is_a_typed_error_instead_of_a_panic() {
        assert!(matches!(
            silent_buffer(usize::MAX),
            Err(AudioError::Allocation(_))
        ));
    }
}
