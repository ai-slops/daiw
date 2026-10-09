use crate::composition_cache_metrics::{CacheLimits, CacheResident};
use std::mem::size_of;

pub(crate) trait CacheValue {
    fn key(&self) -> &[u8; 32];
    fn note_count(&self) -> usize;
    fn path_entries(&self) -> usize;
    fn payload_bytes(&self) -> usize;
}

pub(crate) struct BoundedLru<V> {
    limits: CacheLimits,
    entries: Vec<V>,
    resident: CacheResident,
}

impl<V: CacheValue> BoundedLru<V> {
    pub(crate) const fn new(limits: CacheLimits) -> Self {
        Self {
            limits,
            entries: Vec::new(),
            resident: CacheResident {
                entries: 0,
                retained_bytes: 0,
                notes: 0,
                path_entries: 0,
            },
        }
    }

    pub(crate) const fn limits(&self) -> CacheLimits {
        self.limits
    }

    pub(crate) const fn resident(&self) -> CacheResident {
        self.resident
    }

    pub(crate) fn get(&mut self, key: &[u8; 32]) -> Option<&V> {
        let index = self.entries.iter().position(|entry| entry.key() == key)?;
        let entry = self.entries.remove(index);
        self.entries.push(entry);
        self.entries.last()
    }

    pub(crate) fn admit(&mut self, entry: V) -> u64 {
        if entry.note_count() > self.limits.max_notes
            || entry.path_entries() > self.limits.max_path_entries
            || size_of::<V>()
                .checked_add(entry.payload_bytes())
                .is_none_or(|bytes| bytes > self.limits.max_retained_bytes)
        {
            return 0;
        }
        self.refresh_resident();
        let mut evictions = 0_u64;
        loop {
            while !self.has_room(&entry) {
                if self.entries.is_empty() {
                    self.compact_storage();
                    break;
                }
                self.entries.remove(0);
                evictions = evictions.saturating_add(1);
                self.refresh_resident();
            }
            if self.entries.len() < self.entries.capacity() {
                break;
            }
            if self.entries.try_reserve_exact(1).is_err() {
                return evictions;
            }
            self.refresh_resident();
            if self.has_room(&entry) {
                break;
            }
            self.compact_storage();
            if self.entries.is_empty() {
                return evictions;
            }
            self.entries.remove(0);
            evictions = evictions.saturating_add(1);
            self.refresh_resident();
        }
        self.entries.push(entry);
        self.refresh_resident();
        evictions
    }

    fn has_room(&self, entry: &V) -> bool {
        self.entries.len() < self.limits.max_entries
            && self
                .resident
                .notes
                .checked_add(entry.note_count())
                .is_some_and(|total| total <= self.limits.max_notes)
            && self
                .resident
                .path_entries
                .checked_add(entry.path_entries())
                .is_some_and(|total| total <= self.limits.max_path_entries)
            && self
                .resident
                .retained_bytes
                .checked_add(entry.payload_bytes())
                .is_some_and(|total| total <= self.limits.max_retained_bytes)
    }

    fn compact_storage(&mut self) {
        self.entries = std::mem::take(&mut self.entries)
            .into_boxed_slice()
            .into_vec();
        self.refresh_resident();
    }

    fn refresh_resident(&mut self) {
        let retained_bytes = self
            .entries
            .capacity()
            .checked_mul(size_of::<V>())
            .and_then(|storage| {
                self.entries.iter().try_fold(storage, |total, entry| {
                    total.checked_add(entry.payload_bytes())
                })
            });
        let notes = self.entries.iter().try_fold(0_usize, |total, entry| {
            total.checked_add(entry.note_count())
        });
        let path_entries = self.entries.iter().try_fold(0_usize, |total, entry| {
            total.checked_add(entry.path_entries())
        });
        if let (Some(retained_bytes), Some(notes), Some(path_entries)) =
            (retained_bytes, notes, path_entries)
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
