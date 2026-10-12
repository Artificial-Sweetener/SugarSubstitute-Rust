// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate seed/summary consistency while preserving supplied M1 attribution unchanged.
use super::{
    BuildIdentity, CaptureEvidence, FailureEvidence, IncidentError, IncidentKey, IncidentRevision,
    IncidentSubject, IncidentUtc, PayloadDisposition, ReportDisposition, RetentionFacts,
    SymbolOutcome,
};
use crate::lifecycle::DesktopExitKind;

/// Last bounded launch/lifecycle stage supplied by the seed owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IncidentStage {
    /// Supervisor seed precedes its fallible initialization.
    SupervisorInitialization,
    /// Desktop seed precedes its fallible initialization.
    DesktopInitialization,
    /// Startup remains active.
    Startup,
    /// Application had reached running state.
    Running,
    /// Explicit shutdown was in progress; this is not completion proof.
    Shutdown,
}
/// Pre-initialization/recovery seed content; construction is not durable publication proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IncidentSeed {
    key: IncidentKey,
    revision: IncidentRevision,
    build: BuildIdentity,
    created: IncidentUtc,
    stage: IncidentStage,
}
impl IncidentSeed {
    /// Retain bounded subject/build/time/stage without authenticating a launch.
    /// # Errors
    /// Rejects a desktop initialization stage for supervisor or the reverse.
    pub fn new(
        key: IncidentKey,
        revision: IncidentRevision,
        build: BuildIdentity,
        created: IncidentUtc,
        stage: IncidentStage,
    ) -> Result<Self, IncidentError> {
        if matches!(
            (key.subject(), stage),
            (IncidentSubject::Desktop(_), IncidentStage::SupervisorInitialization)
                | (IncidentSubject::Supervisor(_), IncidentStage::DesktopInitialization)
        ) {
            return Err(IncidentError::MismatchedFacts);
        }
        Ok(Self { key, revision, build, created, stage })
    }
    /// Return full stable subject identity, independent of seed revisions.
    pub const fn key(&self) -> IncidentKey {
        self.key
    }
    /// Return the immutable seed revision.
    pub const fn revision(&self) -> IncidentRevision {
        self.revision
    }
    /// Borrow exact bounded build identity.
    pub const fn build(&self) -> &BuildIdentity {
        &self.build
    }
    /// Return injected UTC seed creation time.
    pub const fn created(&self) -> IncidentUtc {
        self.created
    }
    /// Return lifecycle stage, which grants no clean-exit or restart authority.
    pub const fn stage(&self) -> IncidentStage {
        self.stage
    }
}
/// External exit attribution supplied by its owner; no status classification occurs here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExitAttribution {
    /// M1 supplied an exact desktop classification and actual native status.
    Desktop {
        /// Authoritative M1 classification, retained unchanged even for native zero.
        kind: DesktopExitKind,
        /// Actual platform status, including zero and negative signal statuses.
        status: i64,
    },
    /// Supervisor's independent observer supplied its actual status, without desktop policy.
    Supervisor {
        /// Actual native status; it proves neither resource settlement nor restart eligibility.
        status: i64,
    },
}
/// Aggregate bounded facts used to construct one immutable incident summary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SummaryFacts {
    retention: RetentionFacts,
    exit: ExitAttribution,
    failure: FailureEvidence,
    capture: CaptureEvidence,
    payload: PayloadDisposition,
    reporting: ReportDisposition,
    symbols: SymbolOutcome,
}
impl SummaryFacts {
    /// Combine independently supplied facts; cross-identity checks occur in the summary.
    pub const fn new(
        retention: RetentionFacts,
        exit: ExitAttribution,
        failure: FailureEvidence,
        capture: CaptureEvidence,
        payload: PayloadDisposition,
        reporting: ReportDisposition,
        symbols: SymbolOutcome,
    ) -> Self {
        Self { retention, exit, failure, capture, payload, reporting, symbols }
    }
    /// Return occurrence/acknowledgement UTC facts independently of presentation.
    pub const fn retention(&self) -> RetentionFacts {
        self.retention
    }
    /// Return supplied authoritative attribution, never reclassifying native status.
    pub const fn exit(&self) -> ExitAttribution {
        self.exit
    }
    /// Return supplied fault mechanism, independent of dump availability.
    pub const fn failure(&self) -> FailureEvidence {
        self.failure
    }
    /// Return actual capture availability supplied by its adapter.
    pub const fn capture(&self) -> CaptureEvidence {
        self.capture
    }
    /// Borrow explicit retained/discarded/quarantined artifact facts.
    pub const fn payload(&self) -> &PayloadDisposition {
        &self.payload
    }
    /// Return reporter attempt facts without acknowledging or restarting anything.
    pub const fn reporting(&self) -> ReportDisposition {
        self.reporting
    }
    /// Borrow exactly one supplied failing-frame result or explicit unresolved state.
    pub const fn symbols(&self) -> &SymbolOutcome {
        &self.symbols
    }
}
/// Validated bounded summary content, separate from actual atomic publication proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IncidentSummary {
    seed: IncidentSeed,
    revision: IncidentRevision,
    facts: SummaryFacts,
}
impl IncidentSummary {
    /// Validate related subject/time/revision/build/dump facts without applying M1 policy.
    ///
    /// The crash codec separately enforces the encoded `MAX_SUMMARY_BYTES` limit.
    /// # Errors
    /// Rejects foreign attribution, regressing occurrence/revision, mismatched resolved
    /// build, or a resolved/raw frame dump/address differing from supplied evidence.
    pub fn new(
        seed: IncidentSeed,
        revision: IncidentRevision,
        facts: SummaryFacts,
    ) -> Result<Self, IncidentError> {
        if revision < seed.revision
            || facts.retention.occurred() < seed.created
            || !matches!(
                (seed.key.subject(), facts.exit),
                (IncidentSubject::Desktop(_), ExitAttribution::Desktop { .. })
                    | (IncidentSubject::Supervisor(_), ExitAttribution::Supervisor { .. })
            )
        {
            return Err(IncidentError::MismatchedFacts);
        }
        let frame_identity = match &facts.symbols {
            SymbolOutcome::Resolved(frame) => {
                if frame.release().build() != seed.build() {
                    return Err(IncidentError::MismatchedFacts);
                }
                Some((frame.dump(), frame.address()))
            }
            SymbolOutcome::Unresolved(unresolved) => {
                unresolved.raw_frame().map(|frame| (frame.dump(), frame.address()))
            }
            SymbolOutcome::NotRequested => None,
        };
        if let Some((dump, frame_address)) = frame_identity
            && (facts.capture != CaptureEvidence::Finished(dump)
                || matches!(facts.failure, FailureEvidence::NativeFault { address, .. } if address != frame_address))
        {
            return Err(IncidentError::MismatchedFacts);
        }
        Ok(Self { seed, revision, facts })
    }
    /// Borrow original seed identity/build; it never disappears on presentation intent.
    pub const fn seed(&self) -> &IncidentSeed {
        &self.seed
    }
    /// Return this summary's expected committed revision, not publication proof.
    pub const fn revision(&self) -> IncidentRevision {
        self.revision
    }
    /// Borrow all validated facts without copying payload or symbol strings.
    pub const fn facts(&self) -> &SummaryFacts {
        &self.facts
    }
}
