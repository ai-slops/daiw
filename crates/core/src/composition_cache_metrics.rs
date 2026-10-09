use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheLimits {
    pub max_entries: usize,
    pub max_retained_bytes: usize,
    pub max_notes: usize,
    pub max_path_entries: usize,
}

impl CacheLimits {
    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            max_entries: 0,
            max_retained_bytes: 0,
            max_notes: 0,
            max_path_entries: 0,
        }
    }

    pub(crate) const fn enabled(self) -> bool {
        self.max_entries > 0
            && self.max_retained_bytes > 0
            && self.max_notes > 0
            && self.max_path_entries > 0
    }
}

impl Default for CacheLimits {
    fn default() -> Self {
        Self {
            max_entries: 64,
            max_retained_bytes: 32 * 1024 * 1024,
            max_notes: 100_000,
            max_path_entries: 500_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheCounters {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub expanded_work: u64,
}

impl CacheCounters {
    pub(crate) fn add_assign(&mut self, other: Self) {
        self.hits = self.hits.saturating_add(other.hits);
        self.misses = self.misses.saturating_add(other.misses);
        self.evictions = self.evictions.saturating_add(other.evictions);
        self.expanded_work = self.expanded_work.saturating_add(other.expanded_work);
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheResident {
    pub entries: usize,
    /// Retained `Vec` backing storage and recursively owned payload capacities.
    /// Excludes allocator metadata, stack values, and process RSS.
    pub retained_bytes: usize,
    pub notes: usize,
    pub path_entries: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheStats {
    pub enabled: bool,
    pub limits: CacheLimits,
    pub resident: CacheResident,
    pub totals: CacheCounters,
    pub last_request: CacheCounters,
}
