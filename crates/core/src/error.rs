use crate::ids::{ModifierId, NoteId, Revision};
use serde::Serialize;
use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, PartialEq, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum NoteTargetIssue {
    #[error("missing note target {note_id}")]
    Missing { note_id: NoteId },
    #[error("ambiguous note target {note_id}")]
    Ambiguous { note_id: NoteId },
    #[error("added note id already exists: {note_id}")]
    AddedIdCollision { note_id: NoteId },
}

#[derive(Clone, Debug, Eq, Error, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BindingMismatch {
    #[error("source definition changed")]
    DefinitionChanged,
    #[error("source revision changed")]
    SourceRevisionChanged,
    #[error("modifier selectors changed")]
    SelectorsChanged,
    #[error("bound slot is missing")]
    SlotMissing,
    #[error("slot range changed")]
    RangeChanged,
    #[error("target cardinality changed")]
    CardinalityChanged,
    #[error("resolved note IDs changed")]
    TargetsChanged,
}

#[derive(Clone, Debug, Eq, Error, PartialEq, Serialize)]
#[error("modifier {modifier_id} requires rebase: {reason}")]
#[serde(rename_all = "camelCase")]
pub struct ModifierBindingIssue {
    pub modifier_id: ModifierId,
    pub reason: BindingMismatch,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot: Option<String>,
}

#[derive(Clone, Debug, Error)]
pub enum CoreError {
    #[error(transparent)]
    NativeFormat(#[from] crate::native_format::NativeError),
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
    #[error("selection handle is stale")]
    SelectionStale,
    #[error("invalid selection handle: {0}")]
    SelectionInvalid(String),
    #[error("selection resolved to {actual} notes, exceeding limit {limit}")]
    SelectionLimitExceeded { limit: usize, actual: usize },
    #[error("unsupported request: {0}")]
    UnsupportedCapability(String),
    #[error("modifier operations conflict on {0}")]
    ModifierConflict(String),
    #[error("invalid modifier ordering: {0}")]
    ModifierOrderingInvalid(String),
    #[error("modifier dependency graph contains a cycle")]
    ModifierOrderingCycle,
    #[error("modifiers {first} and {second} require an explicit dependency path")]
    ModifierOrderRequired {
        first: ModifierId,
        second: ModifierId,
    },
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
    #[error("note patch cannot be rebased: {0}")]
    NoteTarget(#[from] NoteTargetIssue),
    #[error(transparent)]
    ModifierBinding(#[from] ModifierBindingIssue),
    #[error("I/O failed: {0}")]
    Io(String),
    #[error("serialization failed: {0}")]
    Serialization(String),
}

impl CoreError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NativeFormat(error) => error.code(),
            Self::MissingState => "MissingState",
            Self::UnexpectedState => "UnexpectedState",
            Self::UnknownMethod(_) => "UnknownMethod",
            Self::InvalidParams(_) => "InvalidParams",
            Self::InvalidProject(_) => "InvalidProject",
            Self::NotFound(_) => "NotFound",
            Self::StaleRevision { .. } => "StaleRevision",
            Self::PermissionDenied => "PermissionDenied",
            Self::SelectionStale => "SelectionStale",
            Self::SelectionInvalid(_) => "SelectionInvalid",
            Self::SelectionLimitExceeded { .. } => "SelectionLimitExceeded",
            Self::UnsupportedCapability(_) => "UnsupportedCapability",
            Self::ModifierConflict(_) => "ModifierConflict",
            Self::ModifierOrderingInvalid(_) => "ModifierOrderingInvalid",
            Self::ModifierOrderingCycle => "ModifierOrderingCycle",
            Self::ModifierOrderRequired { .. } => "ModifierOrderRequired",
            Self::MissingDependency(_) => "MissingDependency",
            Self::StaleProposal => "StaleProposal",
            Self::ProposalClosed => "ProposalClosed",
            Self::HunkAlreadyAccepted(_) => "HunkAlreadyAccepted",
            Self::RebaseConflict(_) | Self::NoteTarget(_) | Self::ModifierBinding(_) => {
                "RebaseConflict"
            }
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
