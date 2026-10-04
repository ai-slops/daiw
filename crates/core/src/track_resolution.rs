use crate::error::CoreError;
use crate::ids::InstanceId;
use crate::music::{Operation, Parameter, Track};
use crate::resolution::ResolvedOperation;

pub fn derive_track(
    base: &Track,
    instance_id: &InstanceId,
    operations: &[ResolvedOperation],
) -> Result<Option<Track>, CoreError> {
    let mut derived = base.clone();
    let mut changed = false;
    for resolved in operations {
        match resolved.operation {
            Operation::Gain { db } => {
                derived.gain += db;
                changed = true;
            }
            Operation::SetParameter {
                ref parameter,
                value,
            } => {
                match parameter {
                    Parameter::Cutoff => derived.cutoff = value,
                    Parameter::Pan => derived.pan = value,
                }
                changed = true;
            }
            Operation::Transpose { .. }
            | Operation::Velocity { .. }
            | Operation::AddNotes { .. }
            | Operation::PatchNotes(_)
            | Operation::ReplaceSlot { .. } => {}
        }
    }
    if !changed {
        return Ok(None);
    }
    if !(-96.0..=24.0).contains(&derived.gain)
        || !(-1.0..=1.0).contains(&derived.pan)
        || !derived.cutoff.is_finite()
        || derived.cutoff <= 0.0
    {
        return Err(CoreError::InvalidProject(format!(
            "effective track for instance {instance_id} is out of range"
        )));
    }
    derived.id = format!("effective:{}:{instance_id}", base.id).into();
    derived.name = format!("{} · {}", base.name, instance_id);
    Ok(Some(derived))
}
