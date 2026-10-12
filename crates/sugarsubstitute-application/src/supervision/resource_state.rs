// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain bounded run, lease, request and native-resource ownership facts.
use std::collections::{BTreeMap, VecDeque};
use sugarsubstitute_contracts::lifecycle::*;

/// Maximum registered run records, including unsettled disconnected runs.
pub const MAX_RUNS: usize = 64;
/// Maximum native resource records, including pending cleanup.
pub const MAX_RESOURCES: usize = 64;
/// Maximum live and pending leases across the coordinator.
pub const MAX_LEASES: usize = 256;
/// Maximum pending requests per registered run.
pub const MAX_RUN_REQUESTS: usize = 64;
/// Maximum pending requests across the coordinator.
pub const MAX_REQUESTS: usize = 256;
/// Maximum exact completed request arguments/outcomes retained per run.
pub const REQUEST_HISTORY: usize = 64;

#[derive(Clone)]
pub(super) struct RequestRecord {
    pub command: ResourceCommand,
    pub reply: ResourceReply,
}
pub(super) struct RunRecord {
    pub registration: ClientRegistration,
    pub incident_seed_admitted: bool,
    pub incident_failure_admitted: bool,
    pub connected: bool,
    pub settled: bool,
    pub highwater: u64,
    pub pending: BTreeMap<RequestId, RequestRecord>,
    pub history: VecDeque<RequestRecord>,
    pub resources: BTreeMap<ResourceId, ResourceKey>,
}
impl RunRecord {
    pub(super) fn finish(&mut self, record: RequestRecord) {
        self.history.push_back(record);
        if self.history.len() > REQUEST_HISTORY {
            self.history.pop_front();
        }
    }
}
/// Observe actual policy phase without claiming cancellation is native settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourcePhase {
    /// Adapter preparation has been requested but claims are not yet observed.
    Preparing,
    /// Native exclusion claim exists; contained process creation remains pending.
    Claimed,
    /// Contained native process is retained; readiness remains pending.
    Starting,
    /// Backend readiness established; independent clients may share leases.
    Ready,
    /// Cleanup was requested; actual ownership remains retained.
    Stopping,
    /// A failure leaves native ownership uncertain and overlap prohibited.
    Uncertain,
    /// Authoritative adapter observed complete native cleanup.
    Settled,
}
pub(super) struct ResourceRecord {
    pub key: ResourceKey,
    pub target: BackendTarget,
    pub phase: ResourcePhase,
    pub claim: bool,
    pub process: Option<ProcessKey>,
    pub leases: BTreeMap<LeaseId, LeaseKey>,
    pub cleanup_requested: bool,
    pub failure: Option<LifecycleFailure>,
}
impl ResourceRecord {
    pub(super) fn cleanup(&mut self) -> Option<ResourceEffect> {
        if self.cleanup_requested || self.phase == ResourcePhase::Settled {
            return None;
        }
        self.cleanup_requested = true;
        if self.failure.is_none() {
            self.phase = ResourcePhase::Stopping;
        }
        Some(match self.target {
            BackendTarget::Managed(_) => ResourceEffect::StopOwned { resource: self.key },
            BackendTarget::Attached(_) => ResourceEffect::DisconnectAttached { resource: self.key },
        })
    }
}
