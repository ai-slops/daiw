use crate::composition_cache_metrics::{CacheCounters, CacheLimits, CacheResident, CacheStats};
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

pub(crate) struct CompositionCache {
    limits: CacheLimits,
    entries: Vec<CacheEntry>,
    resident: CacheResident,
    totals: CacheCounters,
    current: CacheCounters,
    last_request: CacheCounters,
}

pub(crate) struct ResolutionSession<'a> {
    cache: &'a mut CompositionCache,
}

impl<'a> ResolutionSession<'a> {
    pub(crate) const fn new(cache: &'a mut CompositionCache) -> Self {
        Self { cache }
    }

    pub(crate) fn compose(
        &mut self,
        project: &Project,
        root: &Definition,
        work: &mut WorkMeter,
    ) -> Result<Vec<ComposedNote>, CoreError> {
        self.cache.compose(project, root, work)
    }
}

pub(crate) fn uncached_session() -> CompositionCache {
    CompositionCache::new(CacheLimits::disabled())
}

impl CompositionCache {
    pub(crate) fn new(limits: CacheLimits) -> Self {
        Self {
            limits,
            entries: Vec::new(),
            resident: CacheResident::default(),
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
            enabled: self.limits.enabled(),
            limits: self.limits,
            resident: self.resident,
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
        if !self.limits.enabled() {
            return compose_metered(project, root, work);
        }
        let Ok(key) = crate::composition_key::composition_key(project, root) else {
            self.current.misses = self.current.misses.saturating_add(1);
            return self.expand(project, root, work, None);
        };
        if let Some(index) = self.entries.iter().position(|entry| entry.key == key) {
            self.current.hits = self.current.hits.saturating_add(1);
            let entry = self.entries.remove(index);
            let cost = entry.work;
            self.entries.push(entry);
            work.consume_replayed(cost)?;
            return self
                .entries
                .last()
                .map(|entry| entry.notes.clone())
                .ok_or_else(|| CoreError::InvalidProject("composition cache entry lost".into()));
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
        if notes.len() > self.limits.max_notes
            || path_entries > self.limits.max_path_entries
            || minimum_entry > self.limits.max_retained_bytes
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
            .is_none_or(|bytes| bytes > self.limits.max_retained_bytes)
        {
            return;
        }
        self.refresh_resident();
        loop {
            while !self.has_room(notes.len(), path_entries, payload_bytes) {
                if self.entries.is_empty() {
                    self.compact_storage();
                    break;
                }
                self.evict_oldest();
            }
            if self.entries.len() < self.entries.capacity() {
                break;
            }
            if self.entries.try_reserve_exact(1).is_err() {
                return;
            }
            self.refresh_resident();
            if self.has_room(notes.len(), path_entries, payload_bytes) {
                break;
            }
            self.compact_storage();
            if self.entries.is_empty() {
                return;
            }
            self.evict_oldest();
        }
        self.entries.push(CacheEntry {
            key,
            notes: retained_notes,
            work,
            payload_bytes,
            path_entries,
        });
        self.refresh_resident();
    }

    fn has_room(&self, notes: usize, paths: usize, payload: usize) -> bool {
        self.entries.len() < self.limits.max_entries
            && self
                .resident
                .notes
                .checked_add(notes)
                .is_some_and(|total| total <= self.limits.max_notes)
            && self
                .resident
                .path_entries
                .checked_add(paths)
                .is_some_and(|total| total <= self.limits.max_path_entries)
            && self
                .resident
                .retained_bytes
                .checked_add(payload)
                .is_some_and(|total| total <= self.limits.max_retained_bytes)
    }

    fn compact_storage(&mut self) {
        self.entries = std::mem::take(&mut self.entries)
            .into_boxed_slice()
            .into_vec();
        self.refresh_resident();
    }

    fn evict_oldest(&mut self) {
        self.entries.remove(0);
        self.current.evictions = self.current.evictions.saturating_add(1);
        self.refresh_resident();
    }

    fn refresh_resident(&mut self) {
        let retained_bytes = self
            .entries
            .capacity()
            .checked_mul(size_of::<CacheEntry>())
            .and_then(|storage| {
                self.entries.iter().try_fold(storage, |total, entry| {
                    total.checked_add(entry.payload_bytes)
                })
            });
        let notes = self
            .entries
            .iter()
            .try_fold(0_usize, |total, entry| total.checked_add(entry.notes.len()));
        let paths = self.entries.iter().try_fold(0_usize, |total, entry| {
            total.checked_add(entry.path_entries)
        });
        if let (Some(retained_bytes), Some(notes), Some(path_entries)) =
            (retained_bytes, notes, paths)
        {
            self.resident = CacheResident {
                entries: self.entries.len(),
                retained_bytes,
                notes,
                path_entries,
            };
        }
    }
}
