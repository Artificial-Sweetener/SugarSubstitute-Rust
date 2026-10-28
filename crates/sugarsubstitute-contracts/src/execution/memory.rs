// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Retain exactly one admitted byte reservation through result ownership.

use std::{
    error::Error,
    fmt,
    sync::{
        Arc, Weak,
        atomic::{AtomicU64, Ordering},
    },
};

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
        Self(Arc::new(ReservationState { bytes, claimed: AtomicU64::new(0), release }))
    }

    /// Return the single shared retained-byte charge.
    pub fn bytes(&self) -> u64 {
        self.0.bytes
    }

    /// Compare the same physical reservation while both original guards remain borrowed.
    /// This allocates nothing, creates no Weak observer, invokes no release port and exposes
    /// no address or admission authority. Equal byte limits do not imply identical custody.
    pub fn same_reservation(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Claim part of the shared charge for one independently owned allocation.
    ///
    /// Claims across every cloned lease atomically share this reservation's byte
    /// limit. A zero-byte claim consumes no capacity but retains the reservation.
    /// Keep the returned guard with its allocation and drop the allocation first.
    /// Dropping a claim makes its capacity reusable; it does not release admission
    /// while any lease or claim still retains the reservation.
    ///
    /// # Errors
    /// Rejects a claim exceeding the remaining charge, including sum overflow.
    pub fn claim(&self, bytes: u64) -> Result<MemoryClaim, MemoryClaimError> {
        self.0
            .claimed
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |claimed| {
                claimed.checked_add(bytes).filter(|total| *total <= self.0.bytes)
            })
            .map_err(|claimed| MemoryClaimError {
                requested_bytes: bytes,
                available_bytes: self.0.bytes - claimed,
            })?;
        Ok(MemoryClaim { lease: self.clone(), bytes })
    }
}

impl fmt::Debug for MemoryLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("MemoryLease").field("bytes", &self.bytes()).finish_non_exhaustive()
    }
}

/// Observe only the identity of the original reservation allocation.
/// The Contracts and Execution owners alone construct this noncloneable observation.
/// It exposes neither a strong lease nor a release port, and invokes no callback.
/// Execution detaches its record observation before returning the original charge.
pub struct MemoryLeaseObservation(Weak<ReservationState>);
impl MemoryLeaseObservation {
    /// Observe an already-owned original ledger without creating admission authority.
    /// Source governance restricts this producer to Contracts and Execution.
    pub fn new(lease: &MemoryLease) -> Self {
        Self(Arc::downgrade(&lease.0))
    }
    /// Compare the actual allocation, without upgrading or projecting its Weak owner.
    pub fn matches_lease(&self, lease: &MemoryLease) -> bool {
        self.0.as_ptr() == Arc::as_ptr(&lease.0)
    }
}

/// Own one allocation's share of an admitted reservation without duplicating it.
///
/// This guard is not cloneable. Shared allocation owners share this same guard,
/// while independently allocated storage must obtain another claim. Allocation
/// storage must drop before its claim so reusable capacity never covers live bytes.
#[must_use = "Keep the byte claim with its independently owned allocation"]
pub struct MemoryClaim {
    lease: MemoryLease,
    bytes: u64,
}

impl MemoryClaim {
    /// Move an existing allocation's charge only after the replacement reservation admits it.
    pub(crate) fn fund_sibling(&self, allocation: &mut Self) -> Result<(), MemoryClaimError> {
        if !self.lease.same_reservation(&allocation.lease) {
            *allocation = self.lease.claim(allocation.bytes)?;
        }
        Ok(())
    }

    /// Claim distinct retained storage from this allocation's original admitted reservation.
    pub(crate) fn claim_sibling(&self, bytes: u64) -> Result<Self, MemoryClaimError> {
        self.lease.claim(bytes)
    }

    /// Return this allocation's claimed capacity in bytes.
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    /// Return the single shared admission reservation in bytes.
    pub fn reserved_bytes(&self) -> u64 {
        self.lease.bytes()
    }
}

impl fmt::Debug for MemoryClaim {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MemoryClaim")
            .field("bytes", &self.bytes)
            .field("reserved_bytes", &self.reserved_bytes())
            .finish_non_exhaustive()
    }
}

impl Drop for MemoryClaim {
    fn drop(&mut self) {
        // Only successful claims construct guards; each noncloneable guard returns
        // exactly its own contribution, so subtraction cannot underflow.
        self.lease.0.claimed.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}

/// Reject an allocation that cannot fit within the shared admitted byte charge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryClaimError {
    /// Capacity requested for the independently owned allocation, in bytes.
    pub requested_bytes: u64,
    /// Unclaimed capacity at the failed atomic admission check, in bytes.
    pub available_bytes: u64,
}

impl fmt::Display for MemoryClaimError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Allocation requires {} admitted bytes, {} remain unclaimed",
            self.requested_bytes, self.available_bytes
        )
    }
}
impl Error for MemoryClaimError {}

/// Couple a byte charge to exactly one final adapter release.
struct ReservationState {
    bytes: u64,
    claimed: AtomicU64,
    release: Box<dyn MemoryRelease>,
}

impl Drop for ReservationState {
    fn drop(&mut self) {
        self.release.release(self.bytes);
    }
}

#[cfg(test)]
#[path = "memory_tests.rs"]
mod tests;
