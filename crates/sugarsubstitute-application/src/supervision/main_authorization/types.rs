// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Expose inert main sequencing facts without filesystem or process privileges.
use std::num::NonZeroU32;
use sugarsubstitute_contracts::{
    execution::Moment,
    lifecycle::{LeaseKey, LifecycleFailure, LifecycleFailureCode},
    startup::MainIdentity,
};

/// Observe one attempt's ordering, independently of native owner settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MainAuthorizationStage {
    /// Requested folder fragments await acknowledgment.
    RequestedFolder,
    /// Complete observed environment awaits checked validation and reply issue.
    Environment,
    /// Actual observed controller PID awaits checked family validation/reply issue.
    Browser,
    /// Both authorization replies were actually issued for the current attempt.
    Authorized,
    /// Admission is closed; native retirement and finite job consumption remain.
    Retiring,
    /// Empty session or a host-confirmed fully retired predecessor.
    Retired,
    /// An original failure closed ordinary admission; cleanup remains separate.
    Failed,
}

/// Supply authenticated request facts or trusted host-validated continuations.
///
/// Construction conveys no native authority. The host must distinguish incoming
/// acknowledgments from real proof completions and actual authorization write issue.
#[derive(Debug)]
pub enum MainAuthorizationObservation {
    /// Acknowledge exact requested-folder progress, measured in UTF16 code units.
    RequestedFolderAccepted {
        /// Exact current main attempt.
        identity: MainIdentity,
        /// Strict next offset, bounded by 32766 units.
        next_offset_units: u32,
    },
    /// Complete actual observed environment folder; root owns fragment validation.
    EnvironmentFolderComplete {
        /// Exact current main attempt.
        identity: MainIdentity,
    },
    /// Root validated its actual current environment proof under the original request.
    EnvironmentVerified {
        /// Exact current main attempt.
        identity: MainIdentity,
    },
    /// Actual current environment authorization write was issued.
    EnvironmentAuthorizationIssued {
        /// Exact current main attempt.
        identity: MainIdentity,
    },
    /// Observe actual positive browser PID after environment authorization issue.
    ControllerObserved {
        /// Exact current main attempt.
        identity: MainIdentity,
        /// Observed PID is a correlation fact, never process-family proof.
        browser_process_id: NonZeroU32,
    },
    /// Root validated the exact actual browser proof under its controller request.
    BrowserVerified {
        /// Exact current main attempt.
        identity: MainIdentity,
        /// Must equal the original actual controller observation.
        browser_process_id: NonZeroU32,
    },
    /// Actual current browser authorization write was issued.
    BrowserAuthorizationIssued {
        /// Exact current main attempt.
        identity: MainIdentity,
        /// Must equal the original actual controller observation.
        browser_process_id: NonZeroU32,
    },
    /// Authenticated exact native main retirement; finite proof jobs remain separate.
    MainRetired {
        /// Exact current main attempt.
        identity: MainIdentity,
    },
    /// Transfer the first original failure; cleanup never resets ordinary admission.
    Failed {
        /// Exact current main attempt.
        identity: MainIdentity,
        /// Original cause remains local, owned and redacted in ordinary formatting.
        failure: LifecycleFailure,
    },
}
impl MainAuthorizationObservation {
    pub(super) const fn identity(&self) -> MainIdentity {
        match self {
            Self::RequestedFolderAccepted { identity, .. }
            | Self::EnvironmentFolderComplete { identity }
            | Self::EnvironmentVerified { identity }
            | Self::EnvironmentAuthorizationIssued { identity }
            | Self::ControllerObserved { identity, .. }
            | Self::BrowserVerified { identity, .. }
            | Self::BrowserAuthorizationIssued { identity, .. }
            | Self::MainRetired { identity }
            | Self::Failed { identity, .. } => *identity,
        }
    }
    pub(super) const fn is_retirement_or_failure(&self) -> bool {
        matches!(self, Self::MainRetired { .. } | Self::Failed { .. })
    }
}

/// Request a separately checked host operation; effects grant no native privileges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MainAuthorizationEffect {
    /// Submit actual environment proof validation for the current request.
    ValidateEnvironment(MainIdentity),
    /// Submit actual browser family validation for the observed controller PID.
    ValidateBrowser {
        /// Exact current main attempt.
        identity: MainIdentity,
        /// Observed PID, not proof or a native handle.
        browser_process_id: NonZeroU32,
    },
    /// Root may issue a reply only after validating its actual environment proof.
    AuthorizeEnvironment(MainIdentity),
    /// Root may issue a reply only after validating its actual retained browser proof.
    AuthorizeBrowser {
        /// Exact current main attempt.
        identity: MainIdentity,
        /// Must match root's actual validated PID/birth/family proof.
        browser_process_id: NonZeroU32,
    },
    /// Join actual main retirement with all finite proof/native owner consumption.
    /// This requests the root's join; it does not establish adapter settlement.
    AcknowledgeRetirement(MainIdentity),
}

/// Copy bounded matching facts; no field proves native authorization or settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MainAuthorizationSnapshot {
    /// Current attempt or none for an empty session.
    pub identity: Option<MainIdentity>,
    /// Full run/resource/connection/lease matching fact.
    pub lease: Option<LeaseKey>,
    /// Unchanged absolute nanosecond deadline in the caller's monotonic epoch.
    pub frontend_until: Option<Moment>,
    /// Current inert sequence stage.
    pub stage: MainAuthorizationStage,
    /// Accepted requested-folder progress in UTF16 units.
    pub requested_offset_units: u32,
    /// One exact observed controller PID, carrying no native privilege.
    pub browser_process_id: Option<NonZeroU32>,
    /// Exact authenticated main retirement was supplied; root join remains separate.
    pub retirement_observed: bool,
    /// Retained original failure classification, if its cause was not transferred.
    pub failure_code: Option<LifecycleFailureCode>,
    /// Sticky cause-observed fence survives transferring the first original cause.
    pub failure_observed: bool,
    /// Highest admitted actual request ID; zero denotes none.
    pub request_highwater: u64,
}
