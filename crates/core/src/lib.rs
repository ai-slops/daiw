mod binding_validation;
mod bounded_lru;
mod change_impact;
mod cli;
mod command;
mod composition_cache;
mod composition_cache_metrics;
#[cfg(test)]
mod composition_cache_tests;
mod composition_key;
mod composition_memory;
#[cfg(test)]
mod composition_replay_tests;
mod definition_composition;
mod definition_graph;
mod definition_mutation;
mod dispatch;
mod dry_run;
mod effective_validation;
mod engine;
mod error;
mod fixture;
mod fixture_values;
mod ids;
mod instance_mutation;
mod instance_pattern_cache;
#[cfg(test)]
mod instance_pattern_cache_tests;
mod instance_resolution;
mod modifier_binding;
mod modifier_plan;
mod music;
mod mutation;
mod mutation_helpers;
mod native_features;
mod native_format;
mod native_loss;
#[cfg(test)]
mod nested_request_tests;
mod note_identity;
mod note_patch;
mod note_resolution;
mod note_selection;
mod note_selector;
mod operation_access;
mod operation_validation;
mod ordering_graph;
mod pattern_growth;
mod pattern_key;
mod pattern_memory;
#[cfg(test)]
mod pattern_ownership_tests;
mod pattern_resolution;
mod project;
mod proposal_adapter;
mod proposal_creation;
mod proposal_model;
mod proposal_preview;
mod proposal_rebase;
mod proposals;
mod protocol;
mod rebase_report;
mod request_params;
mod resolution;
mod resolver;
mod revision;
mod selected_note_edit;
mod time;
mod timing_mutation;
mod track_resolution;
mod validation;
mod worker_runtime;

pub use cli::run as run_cli;
pub use command::{ChangeRequest, Command, EditScope};
pub use composition_cache_metrics::{CacheCounters, CacheLimits, CacheResident, CacheStats};
pub use daiw_timing::{
    Beat, Curve, MAX_BEATS, MAX_TICKS, MeterMap, MeterPoint, TICKS_PER_BEAT, TempoMap, TempoPoint,
    Tick, TimingError,
};
pub use definition_graph::{DefinitionRevision, definition_revisions};
pub use dispatch::dispatch;
pub use engine::{apply_change, preview_change};
pub use error::{BindingMismatch, CoreError, ErrorBody, ModifierBindingIssue, NoteTargetIssue};
pub use fixture::create_project;
pub use ids::*;
pub use modifier_binding::{BoundSlot, ModifierBinding};
pub use music::*;
pub use note_patch::NotePatch;
pub use note_selection::{
    MAX_SELECTION_RESULTS, NoteSelector, QueryNotesParams, SelectionHandle, query_notes,
};
pub use ordering_graph::{ModifierOrderEdge, ModifierOrdering};
pub use project::*;
pub use proposal_model::*;
pub use proposals::{accept_proposal, create_proposal, reject_proposal};
pub use protocol::*;
pub use resolver::snapshot;
pub use revision::{canonical_hash, musical_fingerprint};
pub use selected_note_edit::NoteTransform;
pub use validation::validate_project;
pub use worker_runtime::WorkerRuntime;
