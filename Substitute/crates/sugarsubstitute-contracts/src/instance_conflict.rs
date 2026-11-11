// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Platform-independent conflict decisions and retained native operation boundaries.
use crate::execution::{CompletionWake, JobContext};
use std::{error::Error, path::PathBuf, sync::Arc};

/// An explicit decision made in the application's conflict surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConflictAction {
    /// Request ordinary shutdown of the verified conflicting copy.
    Close,
    /// Explicitly terminate that verified copy after ordinary shutdown stalls.
    Force,
    /// Cancel this launch without terminating another copy.
    Cancel,
}
/// A bounded operation on retained platform-owned process and lock identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConflictOperation {
    /// Verify the conflicting installation and its actual process ownership.
    Discover,
    /// Request normal shutdown of the verified owner.
    Close,
    /// Terminate only the verified owner after the user's explicit decision.
    Force,
    /// Observe process exit and actual lock release.
    Observe,
    /// Release this resolver's native resources without terminating another copy.
    Retire,
}
/// Observations carry information, never process-control authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConflictObservation {
    /// Verified installation location for presentation to the user.
    Owner(PathBuf),
    /// Process or lock custody remains live.
    Waiting,
    /// Retained processes exited and the required exclusions were available.
    Released,
    /// Resolver resources have actually retired.
    Retired,
}
/// The native owner retains partial outputs across cancellation and worker failure.
/// Clones share the same custody; observations and PIDs cannot reconstruct authority.
/// Calls run only in admitted execution jobs, never on the UI thread.
pub trait ConflictBackend: Clone + Send + Sync + 'static {
    /// Original platform failure, preserved through the application boundary.
    type Error: Error + Send + Sync + 'static;
    /// Execute one bounded operation using retained identities.
    /// # Errors
    /// Reject changed/unverified ownership, expired work, and native failures.
    fn execute(
        &self,
        operation: ConflictOperation,
        context: &JobContext,
    ) -> Result<ConflictObservation, Self::Error>;
    /// Start a fresh application process after this resolver has retired.
    /// Retain every partial launch output before any later fallible operation.
    /// # Errors
    /// Preserve preparation/creation errors; never substitute an unverified executable.
    fn relaunch(&self, context: &JobContext) -> Result<(), Self::Error>;
    /// Retire partial relaunch outputs after cancellation or failure.
    /// This must not terminate an already transferred replacement application.
    /// # Errors
    /// Report actual resource-release failures without claiming settlement.
    fn retire_relaunch(&self) -> Result<(), Self::Error>;
}
/// Presentation state does not grant native process privileges.
pub struct ConflictView {
    /// Localized status and verified installation location.
    pub text: String,
    /// Ordinary close is currently available.
    pub close_enabled: bool,
    /// Explicit force termination is currently available.
    pub force_enabled: bool,
    /// Native custody and execution workers have finished retiring.
    pub done: bool,
}
/// UI-thread controller. Every callback returns promptly; work uses the execution owner.
pub trait ConflictController {
    /// Start owned work with the surface's retained thread-safe completion wake.
    fn start(&mut self, wake: Arc<dyn CompletionWake>);
    /// Consume bounded completions and project the current presentation state.
    fn poll(&mut self) -> ConflictView;
    /// Admit an explicit user decision only when the policy permits it.
    fn choose(&mut self, action: ConflictAction);
}
