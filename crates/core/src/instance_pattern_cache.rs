use crate::bounded_lru::{BoundedLru, CacheValue};
use crate::composition_cache_metrics::{CacheCounters, CacheLimits, CacheStats};
use crate::definition_composition::{ComposedNote, WorkMeter};
use crate::error::CoreError;
use crate::music::{Definition, Instance};
use crate::pattern_resolution::PatternNote;
use crate::project::Project;
use crate::resolution::ResolvedOperation;
use std::mem::size_of;

struct PatternEntry {
    key: [u8; 32],
    notes: Vec<PatternNote>,
    work_trace: Vec<usize>,
    payload_bytes: usize,
    path_entries: usize,
}

impl CacheValue for PatternEntry {
    fn key(&self) -> &[u8; 32] {
        &self.key
    }

    fn note_count(&self) -> usize {
        self.notes.len()
    }

    fn path_entries(&self) -> usize {
        self.path_entries
    }

    fn payload_bytes(&self) -> usize {
        self.payload_bytes
    }
}

pub(crate) struct PatternRequest<'a> {
    pub project: &'a Project,
    pub instance: &'a Instance,
    pub definition: &'a Definition,
    pub composed: Vec<ComposedNote>,
    pub operations: &'a [ResolvedOperation],
}

pub(crate) struct InstancePatternCache {
    entries: BoundedLru<PatternEntry>,
    totals: CacheCounters,
    current: CacheCounters,
    last_request: CacheCounters,
}

impl InstancePatternCache {
    pub(crate) const fn new(limits: CacheLimits) -> Self {
        Self {
            entries: BoundedLru::new(limits),
            totals: CacheCounters {
                hits: 0,
                misses: 0,
                evictions: 0,
                expanded_work: 0,
            },
            current: CacheCounters {
                hits: 0,
                misses: 0,
                evictions: 0,
                expanded_work: 0,
            },
            last_request: CacheCounters {
                hits: 0,
                misses: 0,
                evictions: 0,
                expanded_work: 0,
            },
        }
    }

    pub(crate) fn begin_request(&mut self) {
        self.current = CacheCounters::default();
    }

    pub(crate) fn finish_request(&mut self) {
        self.totals.add_assign(self.current);
        self.last_request = self.current;
    }

    pub(crate) const fn stats(&self) -> CacheStats {
        CacheStats {
            enabled: self.entries.limits().enabled(),
            limits: self.entries.limits(),
            resident: self.entries.resident(),
            totals: self.totals,
            last_request: self.last_request,
        }
    }

    pub(crate) fn resolve(
        &mut self,
        request: PatternRequest<'_>,
        work: &mut WorkMeter,
    ) -> Result<Vec<PatternNote>, CoreError> {
        if !self.entries.limits().enabled() {
            return crate::pattern_resolution::from_composed(
                request.composed,
                request.definition,
                request.operations,
                work,
                &mut Vec::new(),
            );
        }
        let Ok(key) = crate::pattern_key::pattern_key(
            request.project,
            request.definition,
            &request.instance.id,
            request.operations,
        ) else {
            self.current.misses = self.current.misses.saturating_add(1);
            return self.expand(request, work, None);
        };
        if let Some(entry) = self.entries.get(&key) {
            self.current.hits = self.current.hits.saturating_add(1);
            for chunk in &entry.work_trace {
                work.consume(*chunk)?;
            }
            drop(request.composed);
            return Ok(entry.notes.clone());
        }
        self.current.misses = self.current.misses.saturating_add(1);
        self.expand(request, work, Some(key))
    }

    fn expand(
        &mut self,
        request: PatternRequest<'_>,
        work: &mut WorkMeter,
        key: Option<[u8; 32]>,
    ) -> Result<Vec<PatternNote>, CoreError> {
        let mut trace = Vec::new();
        let result = crate::pattern_resolution::from_composed(
            request.composed,
            request.definition,
            request.operations,
            work,
            &mut trace,
        );
        let expanded = trace.iter().fold(0_u64, |total, chunk| {
            let value = u64::try_from(*chunk).unwrap_or(u64::MAX);
            total.saturating_add(value)
        });
        self.current.expanded_work = self.current.expanded_work.saturating_add(expanded);
        let notes = result?;
        if let Some(key) = key {
            self.insert(key, &notes, &trace);
        }
        Ok(notes)
    }

    fn insert(&mut self, key: [u8; 32], notes: &[PatternNote], trace: &[usize]) {
        let Some(path_entries) = crate::pattern_memory::path_entries(notes) else {
            return;
        };
        let Some(minimum_payload) =
            crate::pattern_memory::minimum_payload_bytes(notes, trace.len())
        else {
            return;
        };
        let limits = self.entries.limits();
        if notes.len() > limits.max_notes
            || path_entries > limits.max_path_entries
            || size_of::<PatternEntry>()
                .checked_add(minimum_payload)
                .is_none_or(|bytes| bytes > limits.max_retained_bytes)
        {
            return;
        }
        let mut retained_notes = Vec::new();
        if retained_notes.try_reserve_exact(notes.len()).is_err() {
            return;
        }
        retained_notes.extend_from_slice(notes);
        let mut retained_trace = Vec::new();
        if retained_trace.try_reserve_exact(trace.len()).is_err() {
            return;
        }
        retained_trace.extend_from_slice(trace);
        let Some(payload_bytes) = crate::pattern_memory::retained_payload_bytes(
            &retained_notes,
            retained_notes.capacity(),
            retained_trace.capacity(),
        ) else {
            return;
        };
        let evictions = self.entries.admit(PatternEntry {
            key,
            notes: retained_notes,
            work_trace: retained_trace,
            payload_bytes,
            path_entries,
        });
        self.current.evictions = self.current.evictions.saturating_add(evictions);
    }
}
