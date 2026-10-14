// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Carry bounded resource requests separately from trusted adapter observations.
use super::{ConnectionId, LeaseKey, LifecycleFailure, ProcessKey, RequestId, ResourceKey, RunKey};
use crate::{
    configuration::{LaunchConfiguration, LoopbackOrigin, ProfileId},
    startup::LaunchId,
};

/// Match managed resources by resolved launch inputs, attachment by admitted origin.
///
/// Construction grants no trust. The adapter must resolve root/interpreter paths,
/// verify ownership and authenticate the client before admission. Paths/arguments
/// remain exact; an origin never authorizes process termination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackendTarget {
    /// Native adapter-validated resolved root/interpreter, exact arguments and port.
    Managed(LaunchConfiguration),
    /// Externally owned server; only disconnect is permitted.
    Attached(LoopbackOrigin),
}
/// Reserve immutable startup identities before native child initialization.
///
/// Enrollment carries data only. It authenticates no peer and grants no resource
/// request, readiness, process or connection authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunEnrollment {
    /// Current coordinator-scoped desktop generation.
    pub run: RunKey,
    /// Existing startup attempt identity.
    pub launch: LaunchId,
    /// Exclusive browser profile retained until actual settlement.
    pub profile: ProfileId,
}
/// Report an authenticated client binding for an already enrolled run.
///
/// These facts do not authenticate themselves; never decode this as browser data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientRegistration {
    /// Fresh scoped run.
    pub run: RunKey,
    /// Existing startup attempt identity.
    pub launch: LaunchId,
    /// Exclusive browser profile, independent of backend resource matching.
    pub profile: ProfileId,
    /// Authenticated native connection identity.
    pub connection: ConnectionId,
}
/// Separately authenticated peer context supplied by the native IPC adapter.
/// Construction retains facts only; never reconstruct this context from request bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClientBinding {
    /// Authenticated current desktop run.
    pub run: RunKey,
    /// Authenticated native connection.
    pub connection: ConnectionId,
}
/// Request a resource operation without any native process authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResourceCommand {
    /// Obtain a shared managed lease or attachment lease.
    Acquire(BackendTarget),
    /// Release this connection's exact lease.
    Release(LeaseKey),
    /// Retain this exact lease across an authorized frontend reload.
    Reload(LeaseKey),
}
/// Decode operation data; the owner separately checks the registered binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceRequest {
    /// Claimed run; not proof of authentication.
    pub run: RunKey,
    /// Claimed connection; must match the adapter-registered binding.
    pub connection: ConnectionId,
    /// Positive monotonically assigned operation identity.
    pub id: RequestId,
    /// Exact arguments retained for bounded duplicate detection.
    pub command: ResourceCommand,
}
/// Report one admitted request's bounded disposition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestOutcome {
    /// Lease reservation exists, but usable readiness is not established.
    Pending(LeaseKey),
    /// Actual backend readiness established this connection's lease.
    Granted(LeaseKey),
    /// Lease release is recorded; native backend settlement is separate.
    Released,
    /// Reload retained the existing lease without starting or stopping a backend.
    Reloaded(LeaseKey),
    /// Pending preparation was cancelled by lease/client release.
    Cancelled,
    /// Preparation failed; the resource owner retains the original cause.
    Failed,
}
/// Deliver a request result under its exact run/request identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceReply {
    /// Connection-bound run.
    pub run: RunKey,
    /// Original request identity.
    pub request: RequestId,
    /// Recorded bounded result.
    pub outcome: RequestOutcome,
}
/// Ask the native registry to perform one operation; not proof of execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResourceEffect {
    /// Acquire claims before contained launch; native ownership stays in its registry.
    Prepare {
        /// Exact new acquisition generation.
        resource: ResourceKey,
        /// Managed or attached matching inputs.
        target: BackendTarget,
    },
    /// Cancel pending acquisition or stop an owned family, exactly once per resource.
    StopOwned {
        /// Owned backend acquisition lifetime, never a PID.
        resource: ResourceKey,
    },
    /// Disconnect attachment without sending any external process stop request.
    DisconnectAttached {
        /// Attached acquisition lifetime.
        resource: ResourceKey,
    },
}
/// Return bounded replies and effects; duplicate requests return no effects.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResourceTransition {
    /// At most 256 admitted lease/request replies; each retains exact identity.
    pub replies: Vec<ResourceReply>,
    /// At most 64 effects when a run disconnects; ordinary requests emit at most one.
    pub effects: Vec<ResourceEffect>,
}
/// Report actual adapter facts; never accepted as decoded request payloads.
#[derive(Debug)]
pub enum ResourceObservation {
    /// Native exclusion claims actually acquired for managed preparation.
    ClaimAcquired(ResourceKey),
    /// Creation-time containment and registry ownership actually established.
    OwnedStarted {
        /// Managed acquisition whose registry binding was established.
        resource: ResourceKey,
        /// Exact native registry entry, not a desktop PID.
        process: ProcessKey,
    },
    /// Bounded backend checks actually established the admitted origin.
    Ready {
        /// Acquisition lifetime.
        resource: ResourceKey,
        /// Verified endpoint.
        origin: LoopbackOrigin,
    },
    /// Preparation/cleanup failed; native ownership remains retained until settlement.
    Failed {
        /// Acquisition lifetime.
        resource: ResourceKey,
        /// Original owned native cause.
        failure: LifecycleFailure,
    },
    /// No pending creation, retained claim, native family or pipe work remains.
    /// Cancelled execution, a successful kill request or a dropped handle is insufficient.
    Settled(ResourceKey),
}
impl ResourceObservation {
    /// Return the acquisition lifetime whose native facts are being observed.
    pub const fn resource(&self) -> ResourceKey {
        match self {
            Self::ClaimAcquired(key) | Self::Settled(key) => *key,
            Self::OwnedStarted { resource, .. }
            | Self::Ready { resource, .. }
            | Self::Failed { resource, .. } => *resource,
        }
    }
}
