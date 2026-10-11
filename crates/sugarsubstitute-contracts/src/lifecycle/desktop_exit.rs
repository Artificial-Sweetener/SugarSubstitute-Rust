// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Separate authenticated completion, actual native exit and resource settlement.
use super::{LifecycleFailure, LifecycleRejection, ProcessKey, RunKey, ShutdownId};
use crate::execution::Moment;
use std::time::Duration;
/// One injected monotonic transaction budget, never serialized as a global clock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShutdownBudget {
    /// Original transaction start in the composing owner's clock epoch.
    pub started: Moment,
    /// Absolute end of the ten-second graceful interval.
    pub graceful_until: Moment,
    /// Absolute end of the fifteen-second force/reap/join interval.
    pub settle_until: Moment,
}
impl ShutdownBudget {
    /// Freeze deadlines once; overflowing monotonic arithmetic rejects admission.
    pub fn new(started: Moment) -> Result<Self, LifecycleRejection> {
        let graceful_until = started
            .checked_add(Duration::from_secs(10))
            .map_err(|_| LifecycleRejection::InvalidClock)?;
        let settle_until = started
            .checked_add(Duration::from_secs(15))
            .map_err(|_| LifecycleRejection::InvalidClock)?;
        Ok(Self { started, graceful_until, settle_until })
    }
}
/// Attribute desktop exit without equating it to supervisor resource settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopExitKind {
    /// Matching current authenticated completion and expected native status.
    Normal,
    /// Matching explicitly cancelled startup completion and expected native status.
    StartupCancelled,
    /// Matching readiness failure completion and expected native status.
    StartupFailed,
    /// No matching completion proof, or unexpected native exit status.
    Unexpected,
}
/// Describe an authenticated intent's expected application completion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionIntent {
    /// Deliberate normal application shutdown.
    Normal,
    /// Explicit startup cancellation.
    StartupCancelled,
    /// A known startup readiness failure.
    StartupFailed,
}
/// Distinct trusted facts admitted by the desktop exit owner.
#[derive(Debug)]
pub enum DesktopObservation {
    /// Current run's authenticated completed teardown evidence.
    Completed {
        /// Exact run.
        run: RunKey,
        /// Matching shutdown transaction.
        shutdown: ShutdownId,
    },
    /// Native process registry observed actual exit, independently of UI callbacks.
    Exited {
        /// Exact retained native identity.
        process: ProcessKey,
        /// Actual native exit code; zero alone proves nothing.
        code: i64,
    },
    /// Supervisor observed all released claims/families/pipes/jobs actually settled.
    ResourcesSettled {
        /// Exact run.
        run: RunKey,
        /// Matching shutdown transaction.
        shutdown: ShutdownId,
    },
    /// Cleanup failed or remains uncertain; original cause stays owned.
    Failed {
        /// Exact run.
        run: RunKey,
        /// Matching shutdown transaction.
        shutdown: ShutdownId,
        /// Original typed cleanup cause.
        failure: LifecycleFailure,
    },
}
