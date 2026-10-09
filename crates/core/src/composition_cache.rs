use crate::bounded_lru::{BoundedLru, CacheValue};
use crate::composition_cache_metrics::{CacheCounters, CacheLimits, CacheStats};
use crate::definition_composition::{ComposedNote, WorkMeter, compose_metered};
use crate::error::CoreError;
use crate::music::Definition;
use crate::project::Project;
use std::mem::size_of;

struct CacheEntry {
    key: [u8; 32],
    notes: Vec<ComposedNote>,
    work: u64,
    payload_bytes: usize,
    path_entries: usize,
}

impl CacheValue for CacheEntry {
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

pub(crate) struct CompositionCache {
    entries: BoundedLru<CacheEntry>,
    totals: CacheCounters,
    current: CacheCounters,
    last_request: CacheCounters,
}

pub(crate) struct ResolutionSession<'a> {
    cache: &'a mut CompositionCache,
    patterns: Option<&'a mut crate::instance_pattern_cache::InstancePatternCache>,
}

impl<'a> ResolutionSession<'a> {
    pub(crate) const fn new(cache: &'a mut CompositionCache) -> Self {
        Self {
            cache,
            patterns: None,
        }
    }

    pub(crate) const fn with_pattern_cache(
        cache: &'a mut CompositionCache,
        patterns: &'a mut crate::instance_pattern_cache::InstancePatternCache,
    ) -> Self {
        Self {
            cache,
            patterns: Some(patterns),
        }
    }

    pub(crate) fn compose(
        &mut self,
        project: &Project,
        root: &Definition,
        work: &mut WorkMeter,
    ) -> Result<Vec<ComposedNote>, CoreError> {
        self.cache.compose(project, root, work)
    }

    pub(crate) fn pattern(
        &mut self,
        request: crate::instance_pattern_cache::PatternRequest<'_>,
        work: &mut WorkMeter,
    ) -> Result<Vec<crate::pattern_resolution::PatternNote>, CoreError> {
        match &mut self.patterns {
            Some(cache) => cache.resolve(request, work),
            None => crate::pattern_resolution::from_composed(
                request.composed,
                request.definition,
                request.operations,
                work,
                &mut Vec::new(),
            ),
        }
    }
}

pub(crate) fn uncached_session() -> CompositionCache {
    CompositionCache::new(CacheLimits::disabled())
}

impl CompositionCache {
    pub(crate) fn new(limits: CacheLimits) -> Self {
        Self {
            entries: BoundedLru::new(limits),
            totals: CacheCounters::default(),
            current: CacheCounters::default(),
            last_request: CacheCounters::default(),
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

    pub(crate) fn compose(
        &mut self,
        project: &Project,
        root: &Definition,
        work: &mut WorkMeter,
    ) -> Result<Vec<ComposedNote>, CoreError> {
        if !self.entries.limits().enabled() {
            return compose_metered(project, root, work);
        }
        let Ok(key) = crate::composition_key::composition_key(project, root) else {
            self.current.misses = self.current.misses.saturating_add(1);
            return self.expand(project, root, work, None);
        };
        if let Some(entry) = self.entries.get(&key) {
            self.current.hits = self.current.hits.saturating_add(1);
            let cost = entry.work;
            work.consume_replayed(cost)?;
            return Ok(entry.notes.clone());
        }
        self.current.misses = self.current.misses.saturating_add(1);
        self.expand(project, root, work, Some(key))
    }

    fn expand(
        &mut self,
        project: &Project,
        root: &Definition,
        work: &mut WorkMeter,
        key: Option<[u8; 32]>,
    ) -> Result<Vec<ComposedNote>, CoreError> {
        let before = work.used();
        let result = compose_metered(project, root, work);
        let expanded = work.used().saturating_sub(before);
        self.current.expanded_work = self.current.expanded_work.saturating_add(expanded);
        let notes = result?;
        if let Some(key) = key {
            self.insert(key, &notes, expanded);
        }
        Ok(notes)
    }

    fn insert(&mut self, key: [u8; 32], notes: &[ComposedNote], work: u64) {
        let Some(path_entries) = crate::composition_memory::path_entries(notes) else {
            return;
        };
        let Some(minimum_payload) = crate::composition_memory::minimum_payload_bytes(notes) else {
            return;
        };
        let Some(minimum_entry) = size_of::<CacheEntry>().checked_add(minimum_payload) else {
            return;
        };
        if notes.len() > self.entries.limits().max_notes
            || path_entries > self.entries.limits().max_path_entries
            || minimum_entry > self.entries.limits().max_retained_bytes
        {
            return;
        }
        let mut retained_notes = Vec::new();
        if retained_notes.try_reserve_exact(notes.len()).is_err() {
            return;
        }
        retained_notes.extend_from_slice(notes);
        let Some(payload_bytes) = crate::composition_memory::retained_payload_bytes(
            &retained_notes,
            retained_notes.capacity(),
        ) else {
            return;
        };
        if size_of::<CacheEntry>()
            .checked_add(payload_bytes)
            .is_none_or(|bytes| bytes > self.entries.limits().max_retained_bytes)
        {
            return;
        }
        let evictions = self.entries.admit(CacheEntry {
            key,
            notes: retained_notes,
            work,
            payload_bytes,
            path_entries,
        });
        self.current.evictions = self.current.evictions.saturating_add(evictions);
    }
}
