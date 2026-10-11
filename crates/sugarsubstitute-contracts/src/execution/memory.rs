// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Retain exactly one admitted byte reservation through result ownership.

use std::{fmt, sync::Arc};

/// Return one byte charge to the execution adapter after its last guard drops.
///
/// Implementations must be infallible, nonblocking and must not panic. The adapter
/// charges the reservation before constructing its lease; this port only releases
/// that existing charge and cannot allocate or expose a memory pool.
pub trait MemoryRelease: Send + Sync {
    /// Release the previously charged number of bytes exactly once.
    fn release(&self, bytes: u64);
}

/// Share one admitted byte charge through job completion and retained buffers.
///
/// Cloning shares the same charge. Acknowledging terminal delivery does not release
/// it. The adapter receives one release after the last completion/result guard drops.
#[derive(Clone)]
#[must_use = "Keep the memory guard with every retained admitted result"]
pub struct MemoryLease(Arc<ReservationState>);

impl MemoryLease {
    /// Retain a charge already accepted by the real execution admission owner.
    ///
    /// The supplied release port owns returning that charge. Creating a lease does
    /// not itself reserve memory; only the adapter may construct admitted guards.
    pub fn new(bytes: u64, release: Box<dyn MemoryRelease>) -> Self {
        Self(Arc::new(ReservationState { bytes, release }))
    }

    /// Return the single shared retained-byte charge.
    pub fn bytes(&self) -> u64 {
        self.0.bytes
    }
}

impl fmt::Debug for MemoryLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("MemoryLease").field("bytes", &self.bytes()).finish_non_exhaustive()
    }
}

/// Couple a byte charge to exactly one final adapter release.
struct ReservationState {
    bytes: u64,
    release: Box<dyn MemoryRelease>,
}

impl Drop for ReservationState {
    fn drop(&mut self) {
        self.release.release(self.bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct ReleaseCounter(Arc<AtomicU64>);
    impl MemoryRelease for ReleaseCounter {
        fn release(&self, bytes: u64) {
            self.0.fetch_add(bytes, Ordering::SeqCst);
        }
    }

    #[test]
    fn cloned_result_guards_release_one_charge_only_after_last_drop() {
        let released = Arc::new(AtomicU64::new(0));
        let lease = MemoryLease::new(64, Box::new(ReleaseCounter(released.clone())));
        let retained_result = lease.clone();
        let second_result = retained_result.clone();
        assert_eq!(second_result.bytes(), 64);
        drop(lease);
        drop(retained_result);
        assert_eq!(released.load(Ordering::SeqCst), 0);
        drop(second_result);
        assert_eq!(released.load(Ordering::SeqCst), 64);
    }
}
