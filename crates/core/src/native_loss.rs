use crate::native_format::NativeError;
use serde_json::Value;

pub fn check(raw: &Value, typed: &Value, path: &str) -> Result<(), NativeError> {
    match (raw, typed) {
        (Value::Object(raw), Value::Object(typed)) => {
            for (key, value) in raw {
                let next = format!("{path}/{key}");
                if let Some(known) = typed.get(key) {
                    check(value, known, &next)?;
                } else {
                    let instance = path.strip_prefix("/instances/").is_some_and(|index| {
                        !index.is_empty() && index.bytes().all(|byte| byte.is_ascii_digit())
                    });
                    let empty_binding = key == "modifierBindings"
                        && value.as_object().is_some_and(|map| map.is_empty());
                    let empty_order = key == "modifierOrdering" && value.is_null();
                    let definition = path.strip_prefix("/definitions/").is_some_and(|index| {
                        !index.is_empty() && index.bytes().all(|byte| byte.is_ascii_digit())
                    });
                    let empty_children =
                        key == "children" && value.as_array().is_some_and(Vec::is_empty);
                    if !(instance && (empty_binding || empty_order) || definition && empty_children)
                    {
                        return Err(NativeError::UnsupportedField(next));
                    }
                }
            }
        }
        (Value::Array(raw), Value::Array(typed)) => {
            for (index, (raw, typed)) in raw.iter().zip(typed).enumerate() {
                check(raw, typed, &format!("{path}/{index}"))?;
            }
        }
        _ => {}
    }
    Ok(())
}
