use crate::ids::{DefinitionId, HunkId, InstanceId, NoteId, ProposalId, Revision};
use crate::music::Operation;
use crate::project::Project;
use crate::protocol::Diagnostic;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProposalStatus {
    Pending,
    Accepted,
    PartiallyAccepted,
    Rejected,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hunk {
    pub id: HunkId,
    pub title: String,
    pub description: String,
    pub operation: Operation,
    pub dependencies: Vec<HunkId>,
    pub accepted: bool,
    #[serde(default)]
    pub slot_binding: Option<SlotBinding>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotBinding {
    pub slot: String,
    pub note_ids: Vec<NoteId>,
    #[serde(default)]
    pub definition_revision: u64,
    #[serde(default)]
    pub cardinality: usize,
    #[serde(default)]
    pub mode: RebaseMode,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RebaseMode {
    #[default]
    Strong,
    Adaptive,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalAnchor {
    pub definition_id: DefinitionId,
    pub definition_revision: u64,
    pub note_ids: Vec<NoteId>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Proposal {
    pub id: ProposalId,
    pub name: String,
    pub request: String,
    pub base_revision: Revision,
    pub instance_id: InstanceId,
    pub status: ProposalStatus,
    pub hunks: Vec<Hunk>,
    #[serde(default)]
    pub base_anchor: Option<ProposalAnchor>,
    #[serde(default)]
    pub base_fingerprint: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalParams {
    pub expected_revision: Revision,
    pub instance_id: InstanceId,
    pub request: String,
    #[serde(default)]
    pub operations: Option<Vec<Operation>>,
}

#[derive(Clone, Debug)]
pub struct Acceptance {
    pub expected_revision: Revision,
    pub proposal_id: ProposalId,
    pub hunk_ids: Vec<HunkId>,
}

#[derive(Clone, Debug)]
pub struct Rejection {
    pub expected_revision: Revision,
    pub proposal_id: ProposalId,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HunkAcceptanceResult {
    pub hunk_id: HunkId,
    pub applied: bool,
    pub code: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptanceOutcome {
    pub project: Project,
    pub accepted: bool,
    pub hunks: Vec<HunkAcceptanceResult>,
    pub diagnostics: Vec<Diagnostic>,
}
