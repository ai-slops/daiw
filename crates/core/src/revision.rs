use crate::error::CoreError;
use crate::ids::{HistoryId, Revision};
use crate::project::{HistoryEntry, Project};

pub fn canonical_hash(project: &Project) -> Result<String, CoreError> {
    let mut canonical = project.clone();
    canonical.revision = Revision::initial();
    canonical.history.clear();
    let encoded = serde_json::to_vec(&canonical)
        .map_err(|error| CoreError::Serialization(error.to_string()))?;
    Ok(blake3::hash(&encoded).to_hex().to_string())
}

pub fn musical_fingerprint(project: &Project) -> Result<String, CoreError> {
    let mut musical = project.clone();
    musical.revision = Revision::initial();
    musical.history.clear();
    musical.proposals.clear();
    let encoded = serde_json::to_vec(&musical)
        .map_err(|error| CoreError::Serialization(error.to_string()))?;
    Ok(blake3::hash(&encoded).to_hex().to_string())
}

pub fn advance(mut project: Project, author: &str, message: &str) -> Result<Project, CoreError> {
    let revision = Revision::from(format!("rev-{}", uuid::Uuid::new_v4()));
    project.revision = revision.clone();
    project.history.push(HistoryEntry {
        id: HistoryId::new_random(),
        message: message.to_owned(),
        author: author.to_owned(),
        revision,
    });
    Ok(project)
}

pub fn require_revision(project: &Project, actual: &Revision) -> Result<(), CoreError> {
    if project.revision == *actual {
        Ok(())
    } else {
        Err(CoreError::StaleRevision {
            expected: project.revision.clone(),
            actual: actual.clone(),
        })
    }
}
