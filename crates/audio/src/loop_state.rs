use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::LoopRange;

#[derive(Debug)]
pub struct LoopState {
    version: AtomicUsize,
    enabled: AtomicBool,
    start: AtomicUsize,
    end: AtomicUsize,
}

impl LoopState {
    pub const fn new() -> Self {
        Self {
            version: AtomicUsize::new(0),
            enabled: AtomicBool::new(false),
            start: AtomicUsize::new(0),
            end: AtomicUsize::new(0),
        }
    }

    pub fn publish(&self, range: Option<LoopRange>, length: usize) {
        self.version.fetch_add(1, Ordering::SeqCst);
        match range {
            Some(range) => {
                let end = range.end.min(length);
                self.start.store(range.start, Ordering::SeqCst);
                self.end.store(end, Ordering::SeqCst);
                self.enabled.store(range.start < end, Ordering::SeqCst);
            }
            None => self.enabled.store(false, Ordering::SeqCst),
        }
        self.version.fetch_add(1, Ordering::SeqCst);
    }

    pub fn snapshot(&self) -> Option<LoopRange> {
        let before = self.version.load(Ordering::SeqCst);
        if !before.is_multiple_of(2) {
            return None;
        }
        let enabled = self.enabled.load(Ordering::SeqCst);
        let range = LoopRange {
            start: self.start.load(Ordering::SeqCst),
            end: self.end.load(Ordering::SeqCst),
        };
        let after = self.version.load(Ordering::SeqCst);
        if before == after && enabled {
            Some(range)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_in_progress_never_exposes_mixed_loop_range() {
        // Given
        let state = LoopState::new();
        state.publish(Some(LoopRange { start: 2, end: 8 }), 16);

        // When
        state.version.store(1, Ordering::SeqCst);
        state.start.store(4, Ordering::SeqCst);
        state.end.store(12, Ordering::SeqCst);

        // Then
        assert_eq!(state.snapshot(), None);
    }
}
