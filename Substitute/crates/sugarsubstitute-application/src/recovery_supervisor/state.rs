// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe finite grant, peer and retirement phases independently from native ownership.
use sugarsubstitute_contracts::{coordinator_entry::EntrySetupBudget, recovery_supervisor::*};
/// Current policy phase; no variant is a process, peer, handle or acknowledgement capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoverySupervisorPhase {
    /// The execution reservation has not been observed.
    Unadmitted,
    /// The original grant is reserved; durable ACK may be issued after a live owner check.
    Reserved,
    /// ACK plus exact original family/profile retirement admitted one launch effect.
    LaunchReady,
    /// Actual supervisor creation was admitted once.
    LaunchIssued,
    /// Kernel/capability authentication completed for the actual supervisor carrier.
    Authenticated,
    /// The fresh supervisor registered its real handler; old registrations remain degraded.
    Registered,
    /// The separately verified real Desktop child is running.
    Active,
    /// Actual shutdown is requested; independent retirement facts remain required.
    Retiring,
    /// All four actual physical owner retirements were observed.
    Retired,
}
/// One bounded command to the composing adapter; consumption itself performs no native work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoverySupervisorEffect {
    /// Start only the fresh subordinate from its retained once-only admission.
    Launch {
        /// Original report/startup/successor correlation.
        key: RecoveryGrantKey,
        /// Same immutable setup deadline; there is no new interval on retries.
        original: EntrySetupBudget,
    },
    /// Obtain or reuse the sole actual owner shutdown budget and close every retained physical owner.
    BeginShutdown {
        /// Exact delegation whose physical custody remains retained.
        key: RecoveryGrantKey,
    },
}
/// Independent physical retirement observations. Facts alone grant no close or stop authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryRetirement {
    /// Actual admitted supervisor process is terminal, or its never-issued creation owner retired.
    Supervisor,
    /// Complete real Desktop/profile Job inventory is empty and its original family pins closed.
    DesktopFamily,
    /// Actual original authenticated transport, mapping and queued operations retired.
    Transport,
    /// Actual fresh handler is terminal and its registration/source loans retired.
    Capture,
}
impl RecoveryRetirement {
    pub(super) const fn bit(self) -> u8 {
        match self {
            Self::Supervisor => 1,
            Self::DesktopFamily => 2,
            Self::Transport => 4,
            Self::Capture => 8,
        }
    }
}
/// Passive immutable observations for structured diagnostics and localized coverage presentation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoverySupervisorSnapshot {
    /// Exact grant/report/run correlation.
    pub key: RecoveryGrantKey,
    /// Current observed policy phase, independent of native capability ownership.
    pub phase: RecoverySupervisorPhase,
    /// Fresh-family capture cannot erase old registration degradation.
    pub coverage: RecoveryCaptureCoverage,
    /// First actual failure category, even after additional cleanup failures.
    pub first_failure: Option<RecoveryFailureCode>,
    /// Actual authenticated supervisor carrier, distinct from its Desktop.
    pub supervisor: Option<RecoveryProcessIdentity>,
    /// Separately verified real Desktop process; carrier exit cannot impersonate it.
    pub desktop: Option<RecoveryProcessIdentity>,
}
