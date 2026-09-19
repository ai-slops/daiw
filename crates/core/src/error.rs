use crate::ids::Revision;
use serde::Serialize;
use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum CoreError {
    #[error("state is required for this method")]
    MissingState,
    #[error("state is not allowed for project.create")]
    UnexpectedState,
    #[error("unknown method: {0}")]
    UnknownMethod(String),
    #[error("invalid parameters: {0}")]
    InvalidParams(String),
    #[error("invalid project: {0}")]
    InvalidProject(String),
    #[error("entity not found: {0}")]
    NotFound(String),
    #[error("expected revision {expected}, received {actual}")]
    StaleRevision {
        expected: Revision,
        actual: Revision,
    },
    #[error("agents cannot mutate canonical state")]
    PermissionDenied,
    #[error("unsupported request: {0}")]
    UnsupportedCapability(String),
    #[error("modifier operations conflict on {0}")]
    ModifierConflict(String),
    #[error("accepted hunk is missing dependency {0}")]
    MissingDependency(String),
    #[error("proposal was based on a stale revision")]
    StaleProposal,
    #[error("proposal is already accepted or rejected")]
    ProposalClosed,
    #[error("proposal hunk was already accepted: {0}")]
    HunkAlreadyAccepted(String),
    #[error("proposal cannot be rebased: {0}")]
    RebaseConflict(String),
    #[error("I/O failed: {0}")]
    Io(String),
    #[error("serialization failed: {0}")]
    Serialization(String),
}

impl CoreError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::MissingState => "MissingState",
            Self::UnexpectedState => "UnexpectedState",
            Self::UnknownMethod(_) => "UnknownMethod",
            Self::InvalidParams(_) => "InvalidParams",
            Self::InvalidProject(_) => "InvalidProject",
            Self::NotFound(_) => "NotFound",
            Self::StaleRevision { .. } => "StaleRevision",
            Self::PermissionDenied => "PermissionDenied",
            Self::UnsupportedCapability(_) => "UnsupportedCapability",
            Self::ModifierConflict(_) => "ModifierConflict",
            Self::MissingDependency(_) => "MissingDependency",
            Self::StaleProposal => "StaleProposal",
            Self::ProposalClosed => "ProposalClosed",
            Self::HunkAlreadyAccepted(_) => "HunkAlreadyAccepted",
            Self::RebaseConflict(_) => "RebaseConflict",
            Self::Io(_) => "IoError",
            Self::Serialization(_) => "SerializationError",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}

impl From<CoreError> for ErrorBody {
    fn from(error: CoreError) -> Self {
        Self {
            code: error.code().to_owned(),
            message: error.to_string(),
        }
    }
}
