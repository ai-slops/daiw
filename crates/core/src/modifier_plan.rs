use crate::error::CoreError;
use crate::ids::ModifierId;
use crate::music::{Instance, Modifier};
use crate::operation_access::{OperationAccess, describe, requires_order, validate_additions};
use crate::ordering_graph::{ModifierOrderEdge, resolve};
use crate::project::Project;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModifierPlan {
    mode: OrderingMode,
    order: Vec<ModifierId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    edges: Option<Vec<ModifierOrderEdge>>,
    modifiers: Vec<ModifierAccess>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum OrderingMode {
    Legacy,
    Explicit,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModifierAccess {
    modifier_id: ModifierId,
    #[serde(flatten)]
    access: OperationAccess,
}

pub fn ordered<'a>(
    project: &'a Project,
    instance: &Instance,
) -> Result<Vec<&'a Modifier>, CoreError> {
    let graph = instance
        .modifier_ordering
        .as_ref()
        .map(|ordering| resolve(&instance.modifiers, ordering))
        .transpose()?;
    let ids = graph
        .as_ref()
        .map_or(&instance.modifiers, |graph| &graph.order);
    let modifiers: Vec<_> = ids
        .iter()
        .map(|id| {
            project
                .modifiers
                .iter()
                .find(|modifier| modifier.id == *id)
                .ok_or_else(|| CoreError::NotFound(id.to_string()))
        })
        .collect::<Result<_, _>>()?;
    if let Some(graph) = graph {
        validate_additions(
            modifiers
                .iter()
                .flat_map(|modifier| &modifier.operations)
                .chain(&instance.overrides),
        )?;
        let mut missing_order = None;
        for (index, left) in modifiers.iter().enumerate() {
            for right in modifiers.iter().skip(index + 1) {
                let mut needs_order = false;
                for a in &left.operations {
                    for b in &right.operations {
                        needs_order |= requires_order(a, b)?;
                    }
                }
                if needs_order && !graph.relates(&left.id, &right.id) && missing_order.is_none() {
                    missing_order = Some(CoreError::ModifierOrderRequired {
                        first: left.id.clone(),
                        second: right.id.clone(),
                    });
                }
            }
        }
        if let Some(error) = missing_order {
            return Err(error);
        }
    }
    Ok(modifiers)
}

pub fn explain(project: &Project, instance: &Instance) -> Result<ModifierPlan, CoreError> {
    let ordered = ordered(project, instance)?;
    let (mode, edges) = match &instance.modifier_ordering {
        Some(graph) => {
            let mut edges = graph.edges.clone();
            edges.sort();
            (OrderingMode::Explicit, Some(edges))
        }
        None => (OrderingMode::Legacy, None),
    };
    let order = ordered.iter().map(|modifier| modifier.id.clone()).collect();
    let modifiers = ordered
        .iter()
        .map(|modifier| {
            let mut access = OperationAccess::default();
            for operation in &modifier.operations {
                let operation = describe(operation);
                access.reads.extend(operation.reads);
                access.writes.extend(operation.writes);
            }
            ModifierAccess {
                modifier_id: modifier.id.clone(),
                access,
            }
        })
        .collect();
    Ok(ModifierPlan {
        mode,
        order,
        edges,
        modifiers,
    })
}
