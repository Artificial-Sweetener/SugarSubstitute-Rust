// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate fixed capture matching facts independently of native process authority.
use crate::native::NativeObservationError;
use std::{
    error::Error,
    fmt,
    num::{NonZeroU32, NonZeroU64},
};
use sugarsubstitute_contracts::{
    crash::{CaptureBudget, CaptureTarget},
    execution::JobContext,
    incident::BuildIdentity,
    lifecycle::CoordinatorKey,
};

/// Reject malformed matching facts before native acquisition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureAttachDataError {
    /// Process identity must contain a positive PID and kernel creation timestamp.
    InvalidProcess,
    /// Creator and handler must be distinct actual processes.
    SameProcess,
    /// The complete capture target must belong to the expected coordinator.
    ForeignTarget,
    /// Build spelling must satisfy the bounded annotation contract.
    InvalidBuild,
    /// Endpoint must be a bounded generated local named-pipe spelling.
    InvalidEndpoint,
    /// Observed identities must match the original immutable expectation.
    IdentityMismatch,
}
impl fmt::Display for CaptureAttachDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid capture attachment facts: {self:?}")
    }
}
impl Error for CaptureAttachDataError {}

/// Kernel identity matching facts; constructing these values opens no process.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureProcessFacts {
    pid: NonZeroU32,
    birth: NonZeroU64,
}
impl CaptureProcessFacts {
    /// Validate positive PID and creation time in 100ns units since January1,1601 UTC.
    /// # Errors
    /// Returns `InvalidProcess` when either identity field is zero.
    pub fn new(pid: u32, birth_100ns: u64) -> Result<Self, CaptureAttachDataError> {
        Ok(Self {
            pid: NonZeroU32::new(pid).ok_or(CaptureAttachDataError::InvalidProcess)?,
            birth: NonZeroU64::new(birth_100ns).ok_or(CaptureAttachDataError::InvalidProcess)?,
        })
    }
    /// Return the positive kernel PID matching fact.
    pub const fn process_id(self) -> u32 {
        self.pid.get()
    }
    /// Return the exact kernel creation timestamp, in 100ns units.
    pub const fn creation_time_100ns(self) -> u64 {
        self.birth.get()
    }
}

/// Provenance claimed by the source; the native factory independently verifies it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureEndpointProvenance {
    /// The checked original creator-to-handler inherited endpoint handoff.
    CreatorInherited,
}

/// Fixed immutable capture expectation, containing no native capability.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct CaptureAttachExpectation {
    owner: CoordinatorKey,
    target: CaptureTarget,
    build: [u8; 64],
    build_length: u8,
    creator: CaptureProcessFacts,
    handler: CaptureProcessFacts,
    attach: CaptureBudget,
}
impl CaptureAttachExpectation {
    /// Bind the actual capture lifetime to exact build/process facts and original setup budget.
    /// Healthy attachment does not start a shutdown budget.
    /// # Errors
    /// Rejects foreign capture targets, equal PIDs and invalid build annotation spelling.
    pub fn new(
        owner: CoordinatorKey,
        target: CaptureTarget,
        build: &BuildIdentity,
        creator: CaptureProcessFacts,
        handler: CaptureProcessFacts,
        attach: CaptureBudget,
    ) -> Result<Self, CaptureAttachDataError> {
        if target.owner() != owner {
            return Err(CaptureAttachDataError::ForeignTarget);
        }
        if creator.process_id() == handler.process_id() {
            return Err(CaptureAttachDataError::SameProcess);
        }
        let value = build.as_str().as_bytes();
        if value.is_empty()
            || value.len() > 64
            || !value.iter().all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(byte))
        {
            return Err(CaptureAttachDataError::InvalidBuild);
        }
        let mut bytes = [0; 64];
        bytes[..value.len()].copy_from_slice(value);
        Ok(Self {
            owner,
            target,
            build: bytes,
            build_length: value.len() as u8,
            creator,
            handler,
            attach,
        })
    }
    /// Return the complete expected coordinator lifetime.
    pub const fn owner(self) -> CoordinatorKey {
        self.owner
    }
    /// Return the complete capture target without projecting a different purpose.
    pub const fn target(self) -> CaptureTarget {
        self.target
    }
    /// Borrow the exact validated ASCII build annotation.
    pub fn build_bytes(&self) -> &[u8] {
        &self.build[..usize::from(self.build_length)]
    }
    /// Return original creator matching facts, granting no process handle.
    pub const fn creator(self) -> CaptureProcessFacts {
        self.creator
    }
    /// Return original handler matching facts, granting no process handle.
    pub const fn handler(self) -> CaptureProcessFacts {
        self.handler
    }
    /// Return the unchanged original attachment budget in the executor's epoch.
    pub const fn attach_budget(self) -> CaptureBudget {
        self.attach
    }
}
impl fmt::Debug for CaptureAttachExpectation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CaptureAttachExpectation")
            .field("target", &self.target)
            .field("creator", &self.creator)
            .field("handler", &self.handler)
            .field("attach", &self.attach)
            .finish_non_exhaustive()
    }
}

/// Bounded endpoint data supplied by the source; native checks still establish authority.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct CaptureEndpointObservation {
    expected: CaptureAttachExpectation,
    provenance: CaptureEndpointProvenance,
    endpoint: [u16; 256],
    length: u16,
}
impl CaptureEndpointObservation {
    /// Validate exact process/binding facts and a generated local endpoint.
    /// # Errors
    /// Rejects mismatched identities, remote endpoints, empty suffixes, NUL and oversize.
    pub fn new(
        expected: &CaptureAttachExpectation,
        provenance: CaptureEndpointProvenance,
        creator: CaptureProcessFacts,
        handler: CaptureProcessFacts,
        endpoint: &[u16],
    ) -> Result<Self, CaptureAttachDataError> {
        if creator != expected.creator || handler != expected.handler {
            return Err(CaptureAttachDataError::IdentityMismatch);
        }
        if !valid_endpoint(endpoint) {
            return Err(CaptureAttachDataError::InvalidEndpoint);
        }
        let mut fixed = [0; 256];
        fixed[..endpoint.len()].copy_from_slice(endpoint);
        Ok(Self { expected: *expected, provenance, endpoint: fixed, length: endpoint.len() as u16 })
    }
    /// Return immutable matching expectations; this is not a launch pin.
    pub const fn expectation(self) -> CaptureAttachExpectation {
        self.expected
    }
    /// Return the claimed checked initial handoff provenance.
    pub const fn provenance(self) -> CaptureEndpointProvenance {
        self.provenance
    }
    /// Borrow exact bounded endpoint UTF16; formatting never discloses it.
    pub fn endpoint_utf16(&self) -> &[u16] {
        &self.endpoint[..usize::from(self.length)]
    }
}
impl fmt::Debug for CaptureEndpointObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CaptureEndpointObservation")
            .field("expected", &self.expected)
            .field("provenance", &self.provenance)
            .finish_non_exhaustive()
    }
}

/// Preserve the original bounded source failure without displaying endpoint/path data.
#[derive(Clone)]
pub struct CaptureEndpointSourceError(pub(crate) NativeObservationError);
impl CaptureEndpointSourceError {
    /// Retain an original adapter cause under a static operation label.
    pub fn new(stage: &'static str, cause: impl Error + Send + Sync + 'static) -> Self {
        Self(NativeObservationError::new(stage, cause))
    }
}
impl fmt::Debug for CaptureEndpointSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CaptureEndpointSourceError")
            .field("stage", &self.0.stage())
            .finish_non_exhaustive()
    }
}
impl fmt::Display for CaptureEndpointSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Capture endpoint source {} failed", self.0.stage())
    }
}
impl Error for CaptureEndpointSourceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

/// Supply the real registered capture owner's cached endpoint facts off the UI thread.
/// Implementations must retain actual module/process/source custody and stay bounded.
/// The sole native execution factory independently verifies kernel identities and I/O.
pub trait CaptureEndpointSource: Send + Sync {
    /// Read cached facts for this admitted immutable expectation, without loading/connecting.
    /// # Errors
    /// Preserves closing/unavailable/provenance/replay failures at the real source owner.
    fn observe(
        &self,
        expected: CaptureAttachExpectation,
        context: &JobContext,
    ) -> Result<CaptureEndpointObservation, CaptureEndpointSourceError>;
    /// Associate the constructor-private actual retirement with this original source view.
    /// Closing sources must still accept their exact already-admitted association.
    /// # Errors
    /// Rejects stale/foreign reports; failure preserves source custody and its charge.
    fn native_retired(
        &self,
        actual: &super::control::CaptureAttachRetirementReport,
    ) -> Result<(), CaptureEndpointSourceError>;
}

/// Retain the first preparation or retirement failure without conferring native authority.
#[derive(Clone)]
pub enum CaptureAttachFailure {
    /// Invalid immutable data before acquisition.
    Data(CaptureAttachDataError),
    /// Original source owner failure.
    Source(CaptureEndpointSourceError),
    /// Original native acquisition/operation/clock failure and bounded cleanup causes.
    Native(NativeObservationError),
    /// The original admitted capture deadline expired.
    TimedOut,
    /// Cancellation invalidated publication; actual native settlement is separate.
    Cancelled,
    /// Cleanup total expired while checked custody remains retained.
    Uncertain,
}
impl fmt::Debug for CaptureAttachFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CaptureAttachFailure({self})")
    }
}
impl fmt::Display for CaptureAttachFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Data(cause) => cause.fmt(f),
            Self::Source(cause) => cause.fmt(f),
            Self::Native(cause) => write!(f, "Capture attachment {} failed", cause.stage()),
            Self::TimedOut => f.write_str("Original capture attachment deadline expired"),
            Self::Cancelled => f.write_str("Capture attachment was cancelled"),
            Self::Uncertain => f.write_str("Capture cleanup retains unresolved native custody"),
        }
    }
}
impl Error for CaptureAttachFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Data(cause) => Some(cause),
            Self::Source(cause) => Some(cause),
            Self::Native(cause) => Some(cause),
            _ => None,
        }
    }
}

/// Validate only the same bounded generated local endpoint spelling.
/// Matching syntax grants no named-pipe/process/installation authority.
pub(crate) fn valid_endpoint(endpoint: &[u16]) -> bool {
    const PREFIX: [u16; 9] = [92, 92, 46, 92, 112, 105, 112, 101, 92];
    endpoint.len() > PREFIX.len()
        && endpoint.len() <= 256
        && endpoint.starts_with(&PREFIX)
        && endpoint[PREFIX.len()..].iter().all(|unit| {
            *unit <= 127 && (*unit as u8).is_ascii_alphanumeric() || [45, 95].contains(unit)
        })
}
