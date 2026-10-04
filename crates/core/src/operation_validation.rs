use crate::error::CoreError;
use crate::music::{Operation, Parameter};
use crate::validation::{MAX_GAIN_DB, MIN_GAIN_DB, validate_notes};
use std::collections::BTreeSet;

pub fn validate_reusable_operations(operations: &[Operation]) -> Result<(), CoreError> {
    if operations
        .iter()
        .any(|operation| matches!(operation, Operation::PatchNotes(_)))
    {
        return Err(CoreError::UnsupportedCapability(
            "note patches are instance-local edits".to_owned(),
        ));
    }
    validate_operations(operations)
}

pub fn validate_operations(operations: &[Operation]) -> Result<(), CoreError> {
    let mut slots = BTreeSet::new();
    let mut parameters = BTreeSet::new();
    for operation in operations {
        match operation {
            Operation::Transpose { semitones } if !semitones.is_finite() => invalid_operation(),
            Operation::Velocity { factor } if !factor.is_finite() || *factor < 0.0 => {
                invalid_operation()
            }
            Operation::Gain { db } if !(MIN_GAIN_DB..=MAX_GAIN_DB).contains(db) => {
                invalid_operation()
            }
            Operation::AddNotes { notes } => validate_notes(notes),
            Operation::PatchNotes(patch) => patch.validate(),
            Operation::ReplaceSlot { slot, notes } => {
                if !slots.insert(slot) {
                    return Err(CoreError::ModifierConflict(format!("slot:{slot}")));
                }
                validate_notes(notes)
            }
            Operation::SetParameter { parameter, value } => {
                if !parameters.insert(parameter) {
                    return Err(CoreError::ModifierConflict(
                        parameter_name(parameter).to_owned(),
                    ));
                }
                let valid = match parameter {
                    Parameter::Cutoff => value.is_finite() && *value > 0.0,
                    Parameter::Pan => (-1.0..=1.0).contains(value),
                };
                if valid { Ok(()) } else { invalid_operation() }
            }
            Operation::Transpose { .. } | Operation::Velocity { .. } | Operation::Gain { .. } => {
                Ok(())
            }
        }?;
    }
    Ok(())
}

fn invalid_operation() -> Result<(), CoreError> {
    Err(CoreError::InvalidProject(
        "operation contains a non-finite or negative value".to_owned(),
    ))
}

const fn parameter_name(parameter: &Parameter) -> &'static str {
    match parameter {
        Parameter::Cutoff => "parameter:cutoff",
        Parameter::Pan => "parameter:pan",
    }
}
