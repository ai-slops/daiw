use crate::definition_composition::ComposedNote;
use crate::error::CoreError;
use crate::music::{Definition, Instance};
use crate::project::Project;
use crate::resolution::ResolvedOperation;

pub(crate) struct InstanceResolution<'a> {
    pub project: &'a Project,
    pub instance: &'a Instance,
    pub definition: &'a Definition,
    pub composed: &'a [ComposedNote],
}

impl InstanceResolution<'_> {
    pub fn validated_operations(&self) -> Result<Vec<ResolvedOperation>, CoreError> {
        let mut operations = Vec::new();
        for modifier in crate::modifier_plan::ordered(self.project, self.instance)? {
            let modifier_id = &modifier.id;
            if let Some(binding) = self.instance.modifier_bindings.get(modifier_id) {
                binding.check(crate::modifier_binding::BindingCheck {
                    project: self.project,
                    definition: self.definition,
                    modifier,
                    composed: self.composed,
                })?;
            }
            operations.extend(modifier.operations.iter().cloned().map(|operation| {
                ResolvedOperation {
                    operation,
                    modifier_id: Some(modifier_id.clone()),
                }
            }));
        }
        operations.extend(self.instance.overrides.iter().cloned().map(|operation| {
            ResolvedOperation {
                operation,
                modifier_id: None,
            }
        }));
        crate::operation_validation::validate_operations(
            &operations
                .iter()
                .map(|resolved| resolved.operation.clone())
                .collect::<Vec<_>>(),
        )?;
        Ok(operations)
    }
}
