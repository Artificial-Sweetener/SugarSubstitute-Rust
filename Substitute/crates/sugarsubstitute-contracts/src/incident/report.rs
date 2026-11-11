// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Separate scoped presentation intent from durable acknowledgement and M1 restart authority.
use super::{IncidentError, IncidentKey, IncidentRevision, IncidentUtc};
use std::num::NonZeroU64;

/// Positive reporter session matching identity, never a native authentication capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReportSessionId(NonZeroU64);
impl ReportSessionId {
    /// Reject zero before retaining a session identity.
    /// # Errors
    /// Returns `InvalidIdentity` for zero.
    pub fn new(value: u64) -> Result<Self, IncidentError> {
        Ok(Self(NonZeroU64::new(value).ok_or(IncidentError::InvalidIdentity)?))
    }
    /// Return the positive matching session number.
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}
/// Complete incident/revision/session correlation, separately authenticated by its adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReportBinding {
    incident: IncidentKey,
    revision: IncidentRevision,
    session: ReportSessionId,
}
impl ReportBinding {
    /// Retain exact matching facts without authenticating their sender.
    pub const fn new(
        incident: IncidentKey,
        revision: IncidentRevision,
        session: ReportSessionId,
    ) -> Self {
        Self { incident, revision, session }
    }
    /// Return complete incident identity.
    pub const fn incident(self) -> IncidentKey {
        self.incident
    }
    /// Return the viewed expected revision.
    pub const fn revision(self) -> IncidentRevision {
        self.revision
    }
    /// Return the matching reporter session.
    pub const fn session(self) -> ReportSessionId {
        self.session
    }
}
/// User export selection, independent of filesystem/export privileges.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ExportSelection {
    /// Default explicit export of bounded redacted text only.
    #[default]
    TextOnly,
    /// User explicitly consented to include sensitive unsanitized native memory evidence.
    TextAndNativeDump,
}
/// Current supervisor presentation decision, independently rechecked before restart intent admission.
/// Historical capture evidence grants no current restart authority.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReportRestartAvailability {
    /// Current capture/attachment source is unavailable; ordinary dismissal remains supported.
    #[default]
    Unavailable,
    /// The current supervisor can admit its supported fresh-run path.
    Available,
}
impl ReportRestartAvailability {
    /// Return presentation eligibility only; the supervisor must still validate its actual owner.
    pub const fn allows_restart(self) -> bool {
        matches!(self, Self::Available)
    }
}
/// Native reporter intent; no variant acknowledges storage or authorizes a restart.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportAction {
    /// Copy bounded redacted text through the native presentation adapter.
    CopyText,
    /// Request an export after native destination selection and separate authority checks.
    Export(ExportSelection),
    /// Request durable acknowledgement following explicit dismissal.
    Dismiss,
    /// Request acknowledgement, then an independently M1-authorized fresh launch.
    DismissAndRequestRestart,
}
/// Scoped action at an expected committed revision; origin is separately authenticated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReportCommand {
    binding: ReportBinding,
    action: ReportAction,
}
impl ReportCommand {
    /// Bind an intent without changing incident retention or granting native authority.
    pub const fn new(binding: ReportBinding, action: ReportAction) -> Self {
        Self { binding, action }
    }
    /// Return exact expected incident/revision/session.
    pub const fn binding(self) -> ReportBinding {
        self.binding
    }
    /// Return requested action; M1 alone may separately authorize restart.
    pub const fn action(self) -> ReportAction {
        self.action
    }
}
/// Actual storage-owner acknowledgement fact at the committed resulting revision.
/// Construction itself is not proof of storage durability or peer authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DurableAcknowledgement {
    incident: IncidentKey,
    revision: IncidentRevision,
    at: IncidentUtc,
}
impl DurableAcknowledgement {
    /// Retain storage-supplied committed identity/revision/time, never reporter exit status.
    pub const fn new(incident: IncidentKey, revision: IncidentRevision, at: IncidentUtc) -> Self {
        Self { incident, revision, at }
    }
    /// Return exactly the acknowledged incident.
    pub const fn incident(self) -> IncidentKey {
        self.incident
    }
    /// Return its actually committed acknowledgement revision.
    pub const fn revision(self) -> IncidentRevision {
        self.revision
    }
    /// Return actual durable acknowledgement UTC time.
    pub const fn at(self) -> IncidentUtc {
        self.at
    }
}
/// Categorical reporter failure; original native/storage causes remain with adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportFailure {
    /// Reporter could not be launched.
    Launch,
    /// Committed incident could not be loaded.
    Load,
    /// Native presentation failed.
    Presentation,
    /// Reporter exited without authenticated dismissal/acknowledgement.
    UnexpectedExit,
    /// Durable acknowledgement failed.
    Acknowledgement,
    /// User export operation failed.
    Export,
    /// Native teardown failed or retained uncertain ownership.
    Retirement,
}
/// Last actual report attempt, independently of acknowledgement state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportAttempt {
    /// Native adapter actually presented this incident.
    Presented,
    /// This attempt failed and its incident remains independently retained.
    Failed(ReportFailure),
}
/// Bounded numeric attempt/disposition facts; no implicit retry or restart policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReportDisposition {
    attempts: u64,
    last: Option<ReportAttempt>,
}
impl ReportDisposition {
    /// Require an observed last attempt exactly when the supplied count is positive.
    /// # Errors
    /// Returns `MismatchedFacts` for zero with a result, or positive without one.
    pub fn new(attempts: u64, last: Option<ReportAttempt>) -> Result<Self, IncidentError> {
        if (attempts == 0) != last.is_none() {
            return Err(IncidentError::MismatchedFacts);
        }
        Ok(Self { attempts, last })
    }
    /// Return observed attempt count without incrementing it or scheduling work.
    pub const fn attempts(self) -> u64 {
        self.attempts
    }
    /// Return last observed presentation/failure, never acknowledgement proof.
    pub const fn last(self) -> Option<ReportAttempt> {
        self.last
    }
}
/// Bounded actual reporter/storage observations; hard process exit is never acknowledgement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportObservation {
    /// Native presentation adapter supplied an actual scoped observation.
    Presented(ReportBinding),
    /// Storage supplied explicit durable acknowledgement at the resulting revision.
    Acknowledged(DurableAcknowledgement),
    /// Scoped attempt failed; it grants no automatic retry or restart.
    Failed {
        /// Exact attempted view/session, separately authenticated.
        binding: ReportBinding,
        /// Categorical failure; native error objects never cross this boundary.
        cause: ReportFailure,
    },
}
