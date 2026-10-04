use crate::error::CoreError;
use crate::native_features::{SUPPORTED, required};
use crate::project::Project;
use crate::validation::validate_project;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum NativeError {
    #[error("unsupported native format or version")]
    UnsupportedVersion,
    #[error("unsupported required feature: {0}")]
    UnsupportedFeature(String),
    #[error("unsupported project field: {0}")]
    UnsupportedField(String),
    #[error("invalid native document: {0}")]
    Invalid(String),
    #[error("native content hash does not match the project")]
    IntegrityMismatch,
}

impl NativeError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedVersion | Self::UnsupportedFeature(_) | Self::UnsupportedField(_) => {
                "UnsupportedProjectFormat"
            }
            Self::Invalid(_) => "InvalidProject",
            Self::IntegrityMismatch => "CorruptProject",
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Envelope {
    format: String,
    format_version: u32,
    required_features: Vec<String>,
    content_hash: String,
    content: Value,
}

fn sorted(value: Value) -> Result<Value, NativeError> {
    match value {
        Value::Object(fields) => Ok(Value::Object(
            fields
                .into_iter()
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .map(|(key, value)| Ok((key, sorted(value)?)))
                .collect::<Result<_, NativeError>>()?,
        )),
        Value::Array(values) => Ok(Value::Array(
            values.into_iter().map(sorted).collect::<Result<_, _>>()?,
        )),
        Value::Number(number) => {
            if number.is_f64() {
                return Ok(if number.as_f64() == Some(0.0) {
                    Value::from(0.0)
                } else {
                    Value::Number(number)
                });
            }
            if number
                .as_u64()
                .is_some_and(|value| value > 9_007_199_254_740_991)
                || number
                    .as_i64()
                    .is_some_and(|value| value < -9_007_199_254_740_991)
            {
                return Err(NativeError::Invalid(
                    "integer exceeds the portable JSON precision range".into(),
                ));
            }
            Ok(Value::Number(number))
        }
        other => Ok(other),
    }
}

fn content_value(project: &Project) -> Result<Value, CoreError> {
    let value = serde_json::to_value(project)
        .map_err(|error| CoreError::Serialization(error.to_string()))?;
    Ok(sorted(value)?)
}

fn content_hash(content: &Value) -> Result<String, CoreError> {
    let bytes =
        serde_json::to_vec(content).map_err(|error| CoreError::Serialization(error.to_string()))?;
    Ok(format!("blake3:{}", blake3::hash(&bytes).to_hex()))
}

pub fn export(project: &Project) -> Result<Value, CoreError> {
    validate_project(project)?;
    let content = content_value(project)?;
    let envelope = Envelope {
        format: "daiw-project".into(),
        format_version: 1,
        required_features: required(project).into_iter().map(str::to_owned).collect(),
        content_hash: content_hash(&content)?,
        content,
    };
    serde_json::to_value(envelope).map_err(|error| CoreError::Serialization(error.to_string()))
}

pub fn import(document: Value) -> Result<(Project, u32), CoreError> {
    let reserved = [
        "format",
        "formatVersion",
        "requiredFeatures",
        "contentHash",
        "content",
    ];
    let envelope = if reserved.iter().any(|key| document.get(key).is_some()) {
        let envelope: Envelope = serde_json::from_value(document.clone())
            .map_err(|error| NativeError::Invalid(error.to_string()))?;
        if envelope.format != "daiw-project" || envelope.format_version != 1 {
            return Err(NativeError::UnsupportedVersion.into());
        }
        for feature in &envelope.required_features {
            if !SUPPORTED.contains(&feature.as_str()) {
                return Err(NativeError::UnsupportedFeature(feature.clone()).into());
            }
        }
        Some(envelope)
    } else {
        None
    };
    let raw = envelope
        .as_ref()
        .map_or(&document, |envelope| &envelope.content);
    let project: Project = serde_json::from_value(raw.clone())
        .map_err(|error| NativeError::Invalid(error.to_string()))?;
    let content = content_value(&project)?;
    crate::native_loss::check(raw, &content, "")?;
    validate_project(&project)?;
    if let Some(envelope) = envelope {
        let features: Vec<_> = required(&project).into_iter().map(str::to_owned).collect();
        if envelope.required_features != features {
            return Err(NativeError::Invalid(
                "requiredFeatures must equal the sorted, unique content requirements".into(),
            )
            .into());
        }
        if envelope.content_hash != content_hash(&content)? {
            return Err(NativeError::IntegrityMismatch.into());
        }
        Ok((project, 1))
    } else {
        Ok((project, 0))
    }
}
