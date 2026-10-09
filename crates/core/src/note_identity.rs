use crate::error::CoreError;
use crate::ids::{InstanceId, NoteId};

const EFFECTIVE_ID_DOMAIN: &[u8] = b"daiw:nested-effective-note-id:v1\0";

pub fn nested_effective(
    instance_id: &InstanceId,
    loop_index: u64,
    pattern_id: &NoteId,
) -> Result<NoteId, CoreError> {
    let encoded = serde_json::to_vec(&(instance_id.as_str(), loop_index, pattern_id.as_str()))
        .map_err(|error| CoreError::Serialization(error.to_string()))?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(EFFECTIVE_ID_DOMAIN);
    hasher.update(&encoded);
    Ok(format!("nested-effective:{}", hasher.finalize().to_hex()).into())
}
