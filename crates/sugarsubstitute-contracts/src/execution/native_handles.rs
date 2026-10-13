// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Account for fixed symbol outputs independently of their actual native retirement.

use super::{ExecutionId, JobId, ScopeId};
use std::{
    error::Error,
    fmt,
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, AtomicU16, Ordering},
    },
};

/// Select one once-issued share of the fixed symbol transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolNativePartition {
    /// Committed reader outputs, including retained partial failures.
    CommittedReader,
    /// Selected installed-release outputs and their checked retirement.
    SelectedRelease,
    /// Provider outputs retained through actual provider closure.
    Provider,
    /// The original diagnostic transport's output custody.
    Transport,
}
impl SymbolNativePartition {
    const fn index(self) -> usize {
        self as usize
    }
    const fn capacity(self) -> u16 {
        match self {
            Self::CommittedReader => 140,
            Self::SelectedRelease => 96,
            Self::Provider => 4,
            Self::Transport => 20,
        }
    }
}

/// Release accounting accepted by the execution owner; never close native outputs.
///
/// Calls must be infallible, nonblocking and nonpanicking. Implementations only
/// return the original reservation or notify passive counter observation.
pub trait NativeHandleRelease: Send + Sync {
    /// Return the original reservation exactly once after its final guard drops.
    fn release_symbol_reservation(&self);
    /// Notify counter changes without sampling clocks or inspecting native state.
    fn output_custody_changed(&self);
}

struct Partition {
    issued: AtomicBool,
    outputs: AtomicU16,
}
struct Counters {
    partitions: [Partition; 4],
    outputs: AtomicU16,
    high_water: AtomicU16,
}
struct Reservation {
    owner: ExecutionId,
    scope: ScopeId,
    job: JobId,
    counters: Arc<Counters>,
    release: Box<dyn NativeHandleRelease>,
}
impl Drop for Reservation {
    fn drop(&mut self) {
        self.release.release_symbol_reservation();
    }
}

/// Share an actual fixed260 reservation; cloning never issues another partition.
#[derive(Clone)]
#[must_use = "Retain the original accounting lease with admitted native owners"]
pub struct NativeHandleLease(Arc<Reservation>);
impl NativeHandleLease {
    pub(super) fn new(
        owner: ExecutionId,
        scope: ScopeId,
        job: JobId,
        release: Box<dyn NativeHandleRelease>,
    ) -> Self {
        let counters = Arc::new(Counters {
            partitions: std::array::from_fn(|_| Partition {
                issued: AtomicBool::new(false),
                outputs: AtomicU16::new(0),
            }),
            outputs: AtomicU16::new(0),
            high_water: AtomicU16::new(0),
        });
        Self(Arc::new(Reservation { owner, scope, job, counters, release }))
    }
    /// Report both private counter/reservation control allocations in bytes.
    /// This accounting fact creates no lease, partition, output or native authority.
    /// It includes pinned Rust Arc headers and their actual payload alignment.
    pub const fn control_storage_bytes() -> usize {
        #[repr(C)]
        struct Control<T> {
            strong: std::sync::atomic::AtomicUsize,
            weak: std::sync::atomic::AtomicUsize,
            value: T,
        }
        size_of::<Control<Reservation>>() + size_of::<Control<Counters>>()
    }
    /// Issue the named partition once across every cloned lease.
    ///
    /// # Errors
    /// Returns `PartitionAlreadyIssued` when another owner already took this share.
    pub fn take_partition(
        &self,
        partition: SymbolNativePartition,
    ) -> Result<NativeHandleClaim, NativeResourceError> {
        if self.0.counters.partitions[partition.index()].issued.swap(true, Ordering::AcqRel) {
            return Err(NativeResourceError::PartitionAlreadyIssued { partition });
        }
        Ok(NativeHandleClaim { lease: self.clone(), partition })
    }
    /// Return the actual execution lifetime that admitted this reservation.
    pub fn owner_id(&self) -> ExecutionId {
        self.0.owner
    }
    /// Return the original scope, independently of any later cleanup scope.
    pub fn original_scope_id(&self) -> ScopeId {
        self.0.scope
    }
    /// Return the original admitted main job.
    pub fn original_job_id(&self) -> JobId {
        self.0.job
    }
    /// Observe counter-only state weakly without retaining the lease or its release port.
    pub fn observation(&self) -> NativeHandleObservation {
        NativeHandleObservation(Arc::downgrade(&self.0.counters))
    }
}
impl fmt::Debug for NativeHandleLease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeHandleLease")
            .field("owner", &self.owner_id())
            .field("scope", &self.original_scope_id())
            .field("job", &self.original_job_id())
            .finish_non_exhaustive()
    }
}

/// Weakly observe counters whose destruction has no release callback or native effect.
#[derive(Clone)]
pub struct NativeHandleObservation(Weak<Counters>);
impl NativeHandleObservation {
    /// Compare the exact counter allocation registered by actual execution admission.
    /// This does not upgrade reservation ownership or invoke any release callback.
    pub fn matches_lease(&self, lease: &NativeHandleLease) -> bool {
        Weak::ptr_eq(&self.0, &Arc::downgrade(&lease.0.counters))
    }
    /// Return `(outputs in custody, peak outputs)`; a retired counter allocation returns zeroes.
    /// Counts are passive accounting observations and establish no native settlement.
    pub fn snapshot(&self) -> (u16, u16) {
        self.0.upgrade().map_or((0, 0), |c| {
            let outputs = c.outputs.load(Ordering::Acquire);
            (outputs, c.high_water.load(Ordering::Acquire).max(outputs))
        })
    }
}

/// Own one noncloneable fixed partition, preserving original reservation identity.
#[must_use = "Keep this partition with its native owner"]
#[derive(Debug)]
pub struct NativeHandleClaim {
    lease: NativeHandleLease,
    partition: SymbolNativePartition,
}
impl NativeHandleClaim {
    /// Compare the actual shared ledger; numeric identity or capacity cannot substitute.
    pub fn matches_lease(&self, lease: &NativeHandleLease) -> bool {
        Arc::ptr_eq(&self.lease.0, &lease.0)
    }
    /// Return the closed named partition.
    pub const fn partition(&self) -> SymbolNativePartition {
        self.partition
    }
    /// Return this partition's immutable output capacity.
    pub const fn capacity(&self) -> u16 {
        self.partition.capacity()
    }
    /// Return outputs currently retained across this partition's guards.
    pub fn in_custody(&self) -> u16 {
        self.lease.0.counters.partitions[self.partition.index()].outputs.load(Ordering::Acquire)
    }
    /// Reserve before an external output can be produced.
    ///
    /// # Errors
    /// Returns `PartitionCapacity` before effects when the fixed share is exhausted.
    pub fn reserve_output(&self) -> Result<NativeHandleOutput, NativeResourceError> {
        let c = &self.lease.0.counters;
        c.partitions[self.partition.index()]
            .outputs
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < self.capacity()).then_some(count + 1)
            })
            .map_err(|_| NativeResourceError::PartitionCapacity {
                partition: self.partition,
                capacity: self.capacity(),
            })?;
        let total = c.outputs.fetch_add(1, Ordering::AcqRel) + 1;
        c.high_water.fetch_max(total, Ordering::AcqRel);
        let output = NativeHandleOutput { lease: self.lease.clone(), partition: self.partition };
        self.lease.0.release.output_custody_changed();
        Ok(output)
    }
}

/// Account for one produced or reserved output; Drop performs no native close.
/// Concrete owners release this guard only after no-output or actual checked retirement.
#[must_use = "Retain this guard until the physical output has actually retired"]
#[derive(Debug)]
pub struct NativeHandleOutput {
    lease: NativeHandleLease,
    partition: SymbolNativePartition,
}
impl Drop for NativeHandleOutput {
    fn drop(&mut self) {
        let c = &self.lease.0.counters;
        c.partitions[self.partition.index()].outputs.fetch_sub(1, Ordering::AcqRel);
        c.outputs.fetch_sub(1, Ordering::AcqRel);
        self.lease.0.release.output_custody_changed();
    }
}

/// Preserve a categorical original reservation rejection without implying native authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeResourceError {
    /// The immutable share has already been handed to its owner.
    PartitionAlreadyIssued {
        /// Named original partition.
        partition: SymbolNativePartition,
    },
    /// The share has no remaining output credit.
    PartitionCapacity {
        /// Named original partition.
        partition: SymbolNativePartition,
        /// Immutable share size.
        capacity: u16,
    },
    /// A different execution lifetime supplied the guard.
    ForeignOwner {
        /// Receiving owner.
        expected: ExecutionId,
        /// Actual originating owner.
        actual: ExecutionId,
    },
    /// Original scope does not match admitted provenance.
    WrongOriginalScope {
        /// Required original scope.
        expected: ScopeId,
        /// Supplied original scope.
        actual: ScopeId,
    },
    /// Original job does not match admitted provenance.
    WrongOriginalJob {
        /// Required original job.
        expected: JobId,
        /// Supplied original job.
        actual: JobId,
    },
    /// A different immutable partition was supplied.
    WrongPartition {
        /// Required named share.
        expected: SymbolNativePartition,
        /// Supplied named share.
        actual: SymbolNativePartition,
    },
    /// The actual original admission or quota ledger is no longer current.
    OriginalAdmissionRetired,
}
impl fmt::Display for NativeResourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Native output accounting rejected: {self:?}")
    }
}
impl Error for NativeResourceError {}

#[cfg(test)]
#[path = "native_handles_tests.rs"]
mod tests;
