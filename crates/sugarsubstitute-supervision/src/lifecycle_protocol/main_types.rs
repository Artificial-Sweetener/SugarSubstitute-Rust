// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Carry shape-only main context and bounded stage classifications.
use super::{LeaseReference, ProtocolError};
use std::{
    fmt,
    num::{NonZeroU32, NonZeroU64},
};
use sugarsubstitute_contracts::{
    configuration::{BackendDataDisposition, LoopbackOrigin},
    startup::{MainIdentity, validate_frontend_version},
};

/// Borrow a label satisfying the single readiness owner's version grammar.
/// Shape establishes no installed or served-source identity.
#[derive(Clone, Copy, PartialEq)]
pub struct VersionLabel<'a>(&'a str);
impl<'a> VersionLabel<'a> {
    /// Validate nonempty <=64 ASCII version bytes through the shared domain owner.
    /// # Errors
    /// Returns VersionLabel when the shared grammar rejects this input.
    pub fn new(value: &'a str) -> Result<Self, ProtocolError> {
        validate_frontend_version(value).map_err(|_| ProtocolError::VersionLabel)?;
        Ok(Self(value))
    }
    /// Borrow the exact original label with no allocation or normalization.
    pub fn as_str(self) -> &'a str {
        self.0
    }
}
impl fmt::Debug for VersionLabel<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VersionLabel").field("bytes", &self.0.len()).finish_non_exhaustive()
    }
}
/// Copied listener-matching facts; a native connection must independently prove its peer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackendPeer {
    /// Claimed process identifier of the verified Comfy listener.
    pub process_id: NonZeroU32,
    /// Claimed process creation time in 100 ns units since January 1, 1601 UTC.
    pub creation_time_100ns: NonZeroU64,
}

/// Borrow the shape of one lease grant and current main context.
/// Constructors/decoding authenticate no origin, asset version, resource or native profile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrantedContext<'a> {
    /// Claimed exact lease, checked separately against authenticated connection ownership.
    pub lease: LeaseReference,
    /// Current main attempt, checked separately against actual application state.
    pub identity: MainIdentity,
    /// Exact supported loopback origin; shape alone proves no listener ownership.
    pub origin: LoopbackOrigin,
    /// Claimed current served frontend version, awaiting retained source evidence.
    pub frontend: VersionLabel<'a>,
    /// Claimed current served SugarCubes version, awaiting retained source evidence.
    pub sugarcubes: VersionLabel<'a>,
    /// Claimed verified backend peer; native connection qualification remains independent.
    pub backend_peer: BackendPeer,
    /// Existing canonical backend identity correlation, independently authenticated by the
    /// native channel. It grants no backend I/O or process authority.
    pub backend_binding: sugarsubstitute_contracts::lifecycle::BackendBindingDigest,
    /// Canonical correlation retained by the parent installation/source-host owner. The native
    /// transport authenticates its provenance; matching text grants no installation authority.
    pub installation: &'a str,
    /// Parent-native-derived execution policy. This inert data grants no storage authority.
    pub backend_data_disposition: BackendDataDisposition,
}
impl GrantedContext<'_> {
    /// Return the explicit parent-derived policy; no path or environment can substitute it.
    pub const fn backend_data_disposition(&self) -> BackendDataDisposition {
        self.backend_data_disposition
    }
}
/// Identify the native stage without serializing its original cause.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MainStage {
    /// Tag1: effective environment/profile folder validation.
    Environment,
    /// Tag2: actual browser process family validation.
    Browser,
    /// Tag3: native frontend navigation.
    Navigation,
    /// Tag4: current frontend readiness.
    Frontend,
    /// Tag5: native lifetime retirement.
    NativeRetirement,
}
/// Classify a main stage failure without paths, process handles or diagnostic text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MainFailureCode {
    /// Tag1: actual validation failed.
    ValidationFailed,
    /// Tag2: observed identity does not match the admitted owner.
    IdentityMismatch,
    /// Tag3: native browser process was outside the retained desktop family.
    OutsideOwnedFamily,
    /// Tag4: required stage owner could not be observed.
    Unavailable,
    /// Tag5: original stage deadline elapsed.
    Deadline,
    /// Tag6: actual cleanup has failed or remains uncertain.
    NativeCleanupUncertain,
}
/// Retain one current-main stage classification independently of native custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MainStageFailure {
    /// Exact claimed main attempt.
    pub identity: MainIdentity,
    /// Failed native stage.
    pub stage: MainStage,
    /// Bounded classification; original cause remains with its local owner.
    pub failure: MainFailureCode,
}
