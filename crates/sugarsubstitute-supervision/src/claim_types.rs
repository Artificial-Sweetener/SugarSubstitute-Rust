// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Expose matching facts and immutable launch input without native claim handles.
use crate::{ClaimError, WindowsLaunchInput};
use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
};
use sugarsubstitute_contracts::{
    execution::{JobSpec, ScopeSpec, TerminalKind},
    lifecycle::{CoordinatorKey, ResourceKey},
};

/// One admitted initialization reserves core state and 128 bounded native slots.
pub const CLAIM_POOL_BYTES: u64 = 2 * 1024 * 1024 + 128 * 512 * 1024;

/// Identify a resolution retained by its actual originating owner.
#[derive(Clone)]
pub struct ResolutionTicket {
    pub(crate) id: u64,
    pub(crate) origin: Arc<()>,
}
impl fmt::Debug for ResolutionTicket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ResolutionTicket").field(&self.id).finish()
    }
}

/// Supply scheduler-neutral specifications for the registry's existing typed ports.
#[derive(Clone, Copy, Debug)]
pub struct ClaimLaunchJobs {
    /// Independent no-deadline native scope owned by the registry.
    pub native_scope: ScopeSpec,
    /// Finite creation job with sufficient actual input reservation.
    pub creation: JobSpec,
    /// Root observation admitted before creation.
    pub root_watch: JobSpec,
}

/// A canonical, single-use prepared launch whose native proof stays with its claim owner.
pub struct ManagedLaunchInput {
    pub(crate) resource: ResourceKey,
    pub(crate) origin: Arc<()>,
    pub(crate) input: Option<WindowsLaunchInput>,
    pub(crate) prepared: Arc<AtomicU8>,
    pub(crate) proof: Arc<crate::native_claims::Resolution>,
}
impl ManagedLaunchInput {
    /// Return the real creation-job peak reservation in bytes.
    pub fn required_reservation_bytes(&self) -> u64 {
        self.input.as_ref().map_or(0, WindowsLaunchInput::required_reservation_bytes)
    }
}
impl fmt::Debug for ManagedLaunchInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ManagedLaunchInput")
            .field("resource", &self.resource)
            .field("required_bytes", &self.required_reservation_bytes())
            .finish_non_exhaustive()
    }
}
impl Drop for ManagedLaunchInput {
    fn drop(&mut self) {
        let _ = self.prepared.compare_exchange(1, 2, Ordering::AcqRel, Ordering::Acquire);
    }
}

/// Identify the finite operation whose actual retained facts are being reported.
#[derive(Clone, Debug)]
pub enum ClaimTarget {
    /// Installation/account coordinator exclusion and storage.
    Coordinator,
    /// Canonical launch resolution.
    Resolution(ResolutionTicket),
    /// One exact backend acquisition generation.
    Backend(ResourceKey),
}

/// Report native facts independently from cancellation of their finite jobs.
#[derive(Clone, Debug)]
pub enum ClaimObservation {
    /// Canonical namespace and fresh epoch are held by this adapter.
    CoordinatorClaimed(CoordinatorKey),
    /// A canonical resolution is retained and can be claimed.
    LaunchResolved(ResolutionTicket),
    /// The backend exclusion and generation record are held.
    BackendClaimed(ResourceKey),
    /// A checked native release followed the exact registry ownership proof.
    BackendReleased(ResourceKey),
    /// An unclaimed resolution has completed checked handle closure.
    ResolutionRetired(ResolutionTicket),
    /// This adapter has closed its native storage; this is not whole-supervisor settlement.
    NativeClaimsClosed,
    /// A native failure retains its original cause and uncertain ownership.
    Failed {
        /// Exact retained target of the finite operation.
        target: ClaimTarget,
        /// Original unexpected native cause with redacted display.
        cause: ClaimError,
    },
    /// The actual finite job stopped; native acquisition facts may also be present.
    Stopped {
        /// Target whose native ownership remains independent of this terminal fact.
        target: ClaimTarget,
        /// Outcome transferred through the actual typed job handle.
        kind: TerminalKind,
    },
}
