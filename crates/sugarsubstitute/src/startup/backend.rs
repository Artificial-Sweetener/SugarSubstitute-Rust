// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native startup capabilities retained behind an operating-system-independent boundary.
use super::ShutdownInitiation;
use crate::{OutputSnapshot, job_sequence::JobSequence};
use std::{error::Error, num::NonZeroUsize};
use sugarsubstitute_application::startup::StartupSession;
use sugarsubstitute_contracts::{execution::*, lifecycle::*, startup::*};
use sugarsubstitute_desktop::{
    DesktopStartupCommand, DesktopStartupObservation, StartupPreparationStep,
};
use sugarsubstitute_execution::ExecutionClient;
/// Preserve original platform errors across the shared controller.
pub type BackendResult<T> = Result<T, Box<dyn Error>>;

/// Current authenticated grant's matching facts for a selected Comfy connection.
/// These copied values do not authorize a request until its native peer is checked.
#[cfg(all(windows, feature = "native-composition"))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasBackend {
    /// Current main-window identity that received the grant.
    pub identity: MainIdentity,
    /// Current resource and lease generation, fenced during backend recovery.
    pub lease: sugarsubstitute_supervision::lifecycle_protocol::LeaseReference,
    /// Loopback origin established by the parent readiness owner.
    pub origin: sugarsubstitute_contracts::configuration::LoopbackOrigin,
    /// Parent-verified process matching facts, independently checked on each connection.
    pub peer: sugarsubstitute_supervision::lifecycle_protocol::BackendPeer,
}

/// Keep policy together with its platform-owned authentication and allocation guards.
pub trait PreparedStartup {
    /// Borrow shared startup policy without detaching native custody.
    fn session(&self) -> &StartupSession;
    /// Mutate shared policy while the platform guard remains retained.
    fn session_mut(&mut self) -> &mut StartupSession;
    /// Return native source loans only after the desktop reports actual retirement.
    fn release_source(&mut self);
}
/// Native process, readiness, capture and profile adapter used by the shared startup host.
/// Methods run on the UI thread and must return promptly. Blocking work must use the
/// supplied execution owner. Failed operations retain every partial native output;
/// cancellation is not evidence of settlement. Associated guards remain opaque.
pub trait StartupBackend {
    /// Complete authenticated startup custody; matching keys cannot construct this guard.
    type Prepared: PreparedStartup;
    /// Adopt the original execution scope before any fallible native admission.
    fn start(
        &mut self,
        client: &ExecutionClient,
        scope: Scope,
        jobs: &JobSequence,
    ) -> BackendResult<()>;
    /// Consume one immutable completion batch; preserve partial native custody on failure.
    fn consume(
        &mut self,
        batch: &CompletionBatch,
        prepared: Option<&Self::Prepared>,
    ) -> BackendResult<()>;
    /// Advance bounded native prerequisites; Ready requires authenticated configuration and capture admission.
    fn prepare_native_ui(&mut self, max: NonZeroUsize) -> BackendResult<StartupPreparationStep>;
    /// Transfer the complete prepared guard once after actual native readiness.
    fn take_prepared_startup(&mut self) -> Option<Self::Prepared>;
    /// Apply the shared policy effect against the retained configuration and original attempt.
    fn apply(&mut self, effect: StartupEffect, prepared: &mut Self::Prepared) -> BackendResult<()>;
    /// Adopt the original desktop observation before any fallible native handling.
    /// MainRevealed is delivered only after shared policy accepts frontend readiness and reveal.
    fn observe(
        &mut self,
        observation: DesktopStartupObservation,
        prepared: Option<&Self::Prepared>,
    ) -> BackendResult<()>;
    /// Present a changed operating-system accent to SugarCubes; `None` withdraws a host accent.
    /// Called on the UI thread; delivery to a live page is queued, never awaited.
    fn system_accent(
        &mut self,
        accent: Option<sugarsubstitute_contracts::appearance::AccentColor>,
    ) -> BackendResult<()>;
    /// Check that a canvas message belongs to the current admitted panel and allocation.
    #[cfg(feature = "native-composition")]
    fn admits_canvas(&self, identity: MainIdentity, lease: &MemoryLease) -> bool;
    /// Borrow the current authenticated grant; managed backend outages fence this capability.
    #[cfg(all(windows, feature = "native-composition"))]
    fn canvas_backend(&self, identity: MainIdentity) -> Option<CanvasBackend>;
    /// Report whether main-window construction still owns native input or jobs.
    fn main_construction_pending(&self) -> bool;
    /// Borrow immutable text while the complete source guard remains alive.
    fn console_text(&self) -> Option<&dyn sugarsubstitute_desktop::ConsoleText>;
    /// Clone the bounded output view together with its original allocation custody.
    fn transcript(&self) -> Option<OutputSnapshot>;
    /// Transfer one semantically accepted event without manufacturing readiness.
    fn take_startup_event(&mut self) -> BackendResult<Option<StartupEvent>>;
    /// Transfer one native command with its original provenance.
    fn take_command(&mut self) -> BackendResult<Option<DesktopStartupCommand>>;
    /// Borrow the earliest original operation cutoff in the supplied execution clock epoch.
    fn next_deadline(&self) -> Option<Moment>;
    /// Report a retained cleanup failure that prevents ordinary retry admission.
    fn cleanup_failed(&self) -> bool;
    /// Reject a retry until the previous attempt has actually retired.
    fn ensure_retry_ready(&self, identity: MainIdentity) -> BackendResult<()>;
    /// Adopt the next application identity after revalidating native retirement.
    fn retry_attempt(
        &mut self,
        identity: MainIdentity,
        prepared: &mut Self::Prepared,
    ) -> BackendResult<()>;
    /// Advance current frontend replacement while preserving the same configuration guard.
    fn advance_reload(&mut self, prepared: &mut Self::Prepared) -> BackendResult<()>;
    /// Request capture shutdown without substituting a new measurement for pending evidence.
    fn begin_shutdown(&mut self) -> BackendResult<ShutdownInitiation>;
    /// Borrow the original shutdown transaction without sampling or renewing its cutoff.
    fn shutdown_budget(&self) -> Option<ShutdownBudget>;
    /// Close admission and retain the original budget while cleanup remains pollable.
    fn request_shutdown(&mut self, budget: ShutdownBudget) -> BackendResult<()>;
    /// Report settlement only after actual native owners retire.
    fn poll_shutdown(&mut self) -> BackendResult<bool>;
    /// Finish checked teardown after settlement; preserve original errors.
    fn finish_shutdown(&mut self) -> BackendResult<()>;
    /// Classify only known native error types without invoking arbitrary error callbacks.
    fn startup_cause_code(
        cause: &(dyn Error + 'static),
    ) -> sugarsubstitute_desktop::StartupCauseCode;
    /// Project passive capture and failure facts without granting native privileges.
    fn close_diagnostic(
        &self,
        cause: &(dyn Error + 'static),
    ) -> (sugarsubstitute_desktop::StartupCaptureState, sugarsubstitute_desktop::StartupCauseCode);
    /// Admit qualification only after the shared policy accepts the current main window.
    #[cfg(feature = "k1-native-qualification")]
    fn preview_after_main_revealed(
        &mut self,
        identity: sugarsubstitute_contracts::startup::MainIdentity,
        session: &sugarsubstitute_application::startup::StartupSession,
    );
    /// Consume only qualification observations, retaining ordinary observations for policy.
    #[cfg(feature = "k1-native-qualification")]
    fn accept_preview_observation(
        &mut self,
        observation: &mut Option<DesktopStartupObservation>,
    ) -> bool;
}
