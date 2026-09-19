use crate::error::CoreError;
use crate::music::{Operation, Parameter};

pub fn parse_request(request: &str) -> Result<Vec<Operation>, CoreError> {
    let normalized = request.to_lowercase();
    let mut operations = Vec::new();
    if normalized.contains("octave up") {
        operations.push(Operation::Transpose { semitones: 12.0 });
    }
    if normalized.contains("octave down") {
        operations.push(Operation::Transpose { semitones: -12.0 });
    }
    if normalized.contains("softer") {
        operations.push(Operation::Velocity { factor: 0.8 });
    }
    if normalized.contains("louder") {
        operations.push(Operation::Velocity { factor: 1.2 });
    }
    if normalized.contains("brighter") {
        operations.push(Operation::SetParameter {
            parameter: Parameter::Cutoff,
            value: 16_000.0,
        });
    }
    if normalized.contains("humanize") || normalized.contains("velocity") {
        operations.push(Operation::Velocity { factor: 0.94 });
    }
    if operations.is_empty() {
        Err(CoreError::UnsupportedCapability(request.to_owned()))
    } else {
        Ok(operations)
    }
}

pub const fn operation_description(operation: &Operation) -> &'static str {
    match operation {
        Operation::Transpose { .. } => "Transpose notes",
        Operation::Velocity { .. } => "Scale note velocities",
        Operation::Gain { .. } => "Adjust gain",
        Operation::AddNotes { .. } => "Add notes",
        Operation::ReplaceSlot { .. } => "Replace note slot",
        Operation::SetParameter { .. } => "Set instrument parameter",
    }
}
