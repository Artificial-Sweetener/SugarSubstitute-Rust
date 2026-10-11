// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Expose exact registry facts independently of native handles and scheduler jobs.
use crate::RegistryError;
use sugarsubstitute_contracts::{
    execution::{JobId, Moment},
    lifecycle::{CoordinatorKey, ProcessKey, ResourceKey, RunKey, ShutdownBudget, ShutdownId},
};

/// Bind a trusted owned launch to one desktop run or managed backend lifetime.
/// IDs are facts, not authentication or canonical installation ownership proof.
/// Attached/user-owned backends are never admitted to this native registry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FamilyBinding {
    /// Trusted current desktop run; profile settlement is a separate policy fact.
    Desktop(RunKey),
    /// Trusted canonically resolved managed resource and current generation.
    Managed(ResourceKey),
}
impl FamilyBinding {
    pub(crate) const fn owner(self) -> CoordinatorKey {
        match self {
            Self::Desktop(run) => run.owner,
            Self::Managed(resource) => resource.owner,
        }
    }
    pub(crate) const fn desktop(self) -> bool {
        matches!(self, Self::Desktop(_))
    }
}
/// Identify an admitted family without granting handle, PID or URL control authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use = "Retain the exact key until family settlement and observation transfer"]
pub struct LaunchTicket {
    pub(crate) process: ProcessKey,
    pub(crate) binding: FamilyBinding,
}
impl LaunchTicket {
    /// Return the monotonically assigned registry identity.
    pub const fn process(self) -> ProcessKey {
        self.process
    }
    /// Return the trusted caller's original scoped ownership binding.
    pub const fn binding(self) -> FamilyBinding {
        self.binding
    }
}
/// Inspect cached state without calling native APIs or inferring process exit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistrySnapshot {
    /// Exact current registry identity.
    pub process: ProcessKey,
    /// Original owned desktop/backend binding.
    pub binding: FamilyBinding,
    /// A typed creation job has not yet been consumed.
    pub creation_pending: bool,
    /// Actual contained startup was observed; this is not UI/readiness progress.
    pub started: bool,
    /// Launch/resume admission has closed for this exact family.
    pub launch_closed: bool,
    /// Actual root exit code, never interpreted as complete family settlement.
    pub root_exit: Option<u32>,
    /// Most recent actual non-root secondary exit, validated against this family's
    /// exact watch/correlation/PID/creation time. One bounded slot is overwritten
    /// only by a later validated member completion; it remains until record retirement.
    pub last_member_exit: Option<sugarsubstitute_execution::windows::ProcessExit>,
    /// TerminateJobObject actually succeeded, distinct from a stop or force request.
    /// This monotonic fact is retained through native closure until record retirement.
    pub forced: bool,
    /// Most recent complete retained member inventory, including exited members.
    pub retained_members: usize,
    /// An admitted finite cleanup job or secondary process watch remains owned.
    pub cleanup_pending: bool,
    /// Complete cached inventory requires a new secondary observation reservation.
    pub member_watch_required: bool,
    /// Actual handles/work settled; observations may still require transfer.
    pub settled: bool,
    /// First cleanup/creation failure remains owned and observable.
    pub failed: bool,
    /// Fixed stop transaction and its original absolute budget.
    pub shutdown: Option<(ShutdownId, ShutdownBudget)>,
}
impl RegistrySnapshot {
    /// Report policy expiry without claiming termination or resetting its budget.
    pub fn deadline_expired(self, now: Moment) -> bool {
        !self.settled && self.shutdown.is_some_and(|(_, budget)| now >= budget.settle_until)
    }
}
/// Classify an actual root-watch terminal without inferring process exit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RootObservationOutcome {
    /// The exact watch's typed cancellation returned after its native retirement.
    Cancelled,
    /// The exact watch's typed timeout returned after its native retirement.
    TimedOut,
}

/// Transfer one actual fact, distinct from a requested stop/cancel/kill operation.
#[derive(Debug)]
pub enum RegistryObservation {
    /// The creation operation returned after containment and actual resume.
    ContainedStarted {
        /// Exact family identity.
        process: ProcessKey,
    },
    /// The typed native observation proved this root actually exited.
    RootExited {
        /// Exact family identity.
        process: ProcessKey,
        /// Actual u32 status, including valid status259.
        code: u32,
    },
    /// The root watch retired without observing exit; family ownership remains live.
    RootObservationRetired {
        /// Exact retained family identity.
        process: ProcessKey,
        /// Exact execution job whose typed terminal was transferred.
        job: JobId,
        /// Actual cancellation or timeout, never process or family settlement.
        outcome: RootObservationOutcome,
    },
    /// Every retained member exited, membership is empty and owned work/handles closed.
    FamilySettled {
        /// Exact family identity.
        process: ProcessKey,
        /// Original cleanup transaction, if one was required.
        shutdown: Option<ShutdownId>,
    },
    /// First original failure plus bounded cleanup causes; settlement remains separate.
    Failed {
        /// Exact family identity.
        process: ProcessKey,
        /// Original owned cause.
        error: RegistryError,
    },
}
