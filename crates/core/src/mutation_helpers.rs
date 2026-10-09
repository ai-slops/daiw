use crate::error::CoreError;
use crate::ids::{InstanceId, ModifierId, RegionId};
use crate::project::Project;

pub(crate) fn duplicate_instance(
    project: &mut Project,
    instance_id: &InstanceId,
) -> Result<String, CoreError> {
    let mut duplicate = project
        .instances
        .iter()
        .find(|instance| instance.id == *instance_id)
        .cloned()
        .ok_or_else(|| CoreError::NotFound(instance_id.to_string()))?;
    duplicate.id = InstanceId::new_random();
    duplicate.name = format!("{} Copy", duplicate.name);
    duplicate.start = duplicate.start + duplicate.length;
    project.instances.push(duplicate);
    Ok("Duplicated instance".to_owned())
}

pub(crate) fn delete_instance(
    project: &mut Project,
    instance_id: &InstanceId,
) -> Result<String, CoreError> {
    let position = project
        .instances
        .iter()
        .position(|instance| instance.id == *instance_id)
        .ok_or_else(|| CoreError::NotFound(instance_id.to_string()))?;
    project.instances.remove(position);
    Ok("Deleted instance".to_owned())
}

pub(crate) fn remove_modifier(
    project: &mut Project,
    instance_id: &InstanceId,
    modifier_id: &ModifierId,
) -> Result<String, CoreError> {
    let instance = crate::instance_mutation::find_instance_mut(project, instance_id)?;
    instance.modifiers.retain(|id| id != modifier_id);
    instance.modifier_bindings.remove(modifier_id);
    if let Some(ordering) = &mut instance.modifier_ordering {
        ordering
            .edges
            .retain(|edge| edge.before != *modifier_id && edge.after != *modifier_id);
    }
    Ok("Removed modifier".to_owned())
}

pub(crate) fn add_audio_region(
    project: &mut Project,
    asset: crate::project::Asset,
    region: crate::project::AudioRegion,
) -> Result<String, CoreError> {
    if let Some(existing) = project
        .assets
        .iter()
        .find(|existing| existing.id == asset.id)
    {
        if existing != &asset {
            return Err(CoreError::InvalidProject(format!(
                "asset {} metadata conflicts with its content address",
                asset.id
            )));
        }
    } else {
        project.assets.push(asset);
    }
    project.regions.push(region);
    Ok("Added audio region".to_owned())
}

pub(crate) fn remove_audio_region(
    project: &mut Project,
    region_id: &RegionId,
) -> Result<String, CoreError> {
    let position = project
        .regions
        .iter()
        .position(|region| region.id == *region_id)
        .ok_or_else(|| CoreError::NotFound(region_id.to_string()))?;
    project.regions.remove(position);
    Ok("Removed audio region".to_owned())
}
