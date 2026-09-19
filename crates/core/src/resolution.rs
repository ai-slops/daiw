use crate::ids::ModifierId;
use crate::music::Operation;

pub struct ResolvedOperation {
    pub operation: Operation,
    pub modifier_id: Option<ModifierId>,
}
