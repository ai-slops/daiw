use crate::command::Command;
use crate::ids::{HunkId, InstanceId, ProposalId, Revision};
use crate::project::Project;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateParams {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub fixture: Option<bool>,
}

#[derive(Deserialize)]
pub struct ImportParams {
    pub project: Project,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotParams {
    #[serde(default)]
    pub proposal_id: Option<ProposalId>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplainParams {
    pub instance_id: InstanceId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeParams {
    pub expected_revision: Revision,
    pub command: Command,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptParams {
    pub expected_revision: Revision,
    pub proposal_id: ProposalId,
    pub hunk_ids: Vec<HunkId>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RejectParams {
    pub expected_revision: Revision,
    pub proposal_id: ProposalId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreParams {
    pub expected_revision: Revision,
    pub snapshot: Project,
}
