use crate::error::CoreError;
use crate::music::EffectiveNote;
use crate::time::MAX_TICKS;
use std::collections::BTreeSet;

pub fn validate(notes: &[EffectiveNote]) -> Result<(), CoreError> {
    let mut ids = BTreeSet::new();
    for note in notes {
        if !ids.insert(&note.id) {
            return Err(CoreError::InvalidProject(format!(
                "duplicate effective note ID {}",
                note.id
            )));
        }
        if !note.pitch.is_finite() || !(0.0..=1.0).contains(&note.velocity) {
            return Err(CoreError::InvalidProject(format!(
                "effective note {} has non-finite pitch or invalid velocity",
                note.id
            )));
        }
        let end = note.start.ticks().checked_add(note.duration.ticks());
        if note.start.ticks() < 0
            || note.duration.ticks() <= 0
            || end.is_none_or(|end| end > MAX_TICKS)
        {
            return Err(CoreError::InvalidProject(format!(
                "effective note {} timing is out of range",
                note.id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate;
    use crate::{CoreError, Tick, create_project, snapshot};

    #[test]
    fn final_values_reject_nan_infinity_and_unnormalized_velocity() -> Result<(), CoreError> {
        let mut notes = snapshot(&create_project(None, true), None)?.notes;
        for (pitch, velocity) in [
            (f64::NAN, 0.8),
            (f64::INFINITY, 0.8),
            (f64::NEG_INFINITY, 0.8),
            (60.25, f64::NAN),
            (60.25, f64::INFINITY),
            (60.25, -0.1),
            (60.25, 1.1),
        ] {
            notes[0].pitch = pitch;
            notes[0].velocity = velocity;
            assert!(matches!(
                validate(&notes),
                Err(CoreError::InvalidProject(_))
            ));
        }
        Ok(())
    }

    #[test]
    fn invalid_final_timing_is_rejected_without_integer_overflow() -> Result<(), CoreError> {
        let mut notes = snapshot(&create_project(None, true), None)?.notes;
        for (start, duration) in [(0, 0), (-1, 1), (i64::MAX, 1)] {
            notes[0].start = Tick::from_ticks(start);
            notes[0].duration = Tick::from_ticks(duration);
            assert!(matches!(
                validate(&notes),
                Err(CoreError::InvalidProject(_))
            ));
        }
        Ok(())
    }
}
