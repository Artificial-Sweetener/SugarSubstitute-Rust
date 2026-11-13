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

/// Why the child cannot admit a page's canvas request right now.
///
/// Each cause names the fact that failed, so a refused request is answered with an actionable
/// reason instead of an empty failure.
#[cfg(feature = "native-composition")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanvasRefusal {
    /// The application is closing.
    Closing,
    /// The request belongs to a page navigation that is no longer the current one.
    OtherNavigation,
    /// The request carries storage that is not the current page's.
    OtherLifetime,
    /// The page's panel handshake has not completed, so it holds no canvas authority yet.
    PanelNotConfigured,
    /// The page's panel handshake failed, so it holds no canvas authority.
    PanelFailed,
    /// The application is not running, or the page is not composition-hosted.
    NativeCanvasNotGranted,
}

#[cfg(feature = "native-composition")]
impl CanvasRefusal {
    /// Closed diagnostic name, free of paths and content.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Closing => "closing",
            Self::OtherNavigation => "other_navigation",
            Self::OtherLifetime => "other_lifetime",
            Self::PanelNotConfigured => "panel_not_configured",
            Self::PanelFailed => "panel_failed",
            Self::NativeCanvasNotGranted => "native_canvas_not_granted",
        }
    }

    /// The typed `unavailable` failure that carries this reason as its developer detail.
    pub fn failure(self) -> sugarsubstitute_contracts::canvas_native::CanvasFailure {
        use sugarsubstitute_contracts::canvas_native::{
            CanvasFailure, CanvasFailureMessage, CanvasFailureStatus,
        };
        let bare = CanvasFailure::new(CanvasFailureStatus::Unavailable);
        CanvasFailure {
            message: CanvasFailureMessage::new(self.detail()).unwrap_or(bare.message),
            status: CanvasFailureStatus::Unavailable,
        }
    }

    /// The reason in words for the page's developer detail.
    pub const fn detail(self) -> &'static str {
        match self {
            Self::Closing => "The application is closing",
            Self::OtherNavigation => "The request belongs to an earlier page navigation",
            Self::OtherLifetime => "The request does not belong to the current page's storage",
            Self::PanelNotConfigured => "The page's panel handshake has not completed",
            Self::PanelFailed => "The page's panel handshake failed",
            Self::NativeCanvasNotGranted => {
                "The application is not running or the page is not composition-hosted"
            }
        }
    }
}

/// Parent-authenticated correlation available before main construction. A host receives this
/// only through its already authenticated current grant; filesystem and backend I/O remain
/// independently admitted by their owners.
#[cfg(windows)]
#[derive(Clone)]
pub struct WorkspaceContext {
    /// Current native main lifetime.
    pub identity: MainIdentity,
    /// Exact existing selected profile.
    pub profile: sugarsubstitute_contracts::configuration::ProfileId,
    /// Existing installed or protected source-host correlation.
    pub installation: sugarsubstitute_contracts::installation::InstallationId,
    /// Existing admitted target's canonical correlation.
    pub backend: BackendBindingDigest,
    /// Authenticated served origin.
    pub origin: sugarsubstitute_contracts::configuration::LoopbackOrigin,
    /// Current authenticated listener matching facts, checked natively on every transfer.
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
    /// Admit a canvas message only when it belongs to the current admitted panel and allocation.
    ///
    /// # Errors
    /// Names the first fact that fails, in the order the application checks them.
    #[cfg(feature = "native-composition")]
    fn canvas_admission(
        &self,
        identity: MainIdentity,
        lease: &MemoryLease,
    ) -> Result<(), CanvasRefusal>;
    /// Borrow the current authenticated grant; managed backend outages fence this capability.
    #[cfg(all(windows, feature = "native-composition"))]
    fn canvas_backend(&self, identity: MainIdentity) -> Option<CanvasBackend>;
    /// Borrow current authenticated correlation before loading durable workspace state.
    #[cfg(windows)]
    fn workspace_context(
        &self,
        prepared: &Self::Prepared,
    ) -> BackendResult<Option<WorkspaceContext>>;
    /// Supply trusted bootstrap and verified placement before native main construction.
    #[cfg(windows)]
    fn configure_workspace(
        &mut self,
        bootstrap: sugarsubstitute_desktop::SessionBootstrap,
        placement: Option<sugarsubstitute_contracts::desktop_session::wire::SessionPlacement>,
    ) -> BackendResult<()>;
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
    /// Start only the existing capture measurement while retaining the current page for final workspace capture.
    fn begin_workspace_close(&mut self) -> BackendResult<ShutdownInitiation>;
    /// Request capture shutdown without substituting a new measurement for pending evidence.
    fn begin_shutdown(&mut self) -> BackendResult<ShutdownInitiation>;
    /// Borrow the original shutdown transaction without sampling or renewing its cutoff.
    fn shutdown_budget(&self) -> Option<ShutdownBudget>;
    /// The coordinator's newest update status, once it sent one. Presentation only.
    fn update_status(&self) -> Option<&sugarsubstitute_contracts::update::status::UpdateStatus>;
    /// The backend's workflow queue at the coordinator's newest status; unobserved before it.
    fn workflow_queue(&self) -> sugarsubstitute_supervision::lifecycle_protocol::WorkflowQueue;
    /// Take, once per launch, the next start-up update message the coordinator's status states.
    fn take_update_notice(
        &mut self,
    ) -> Option<sugarsubstitute_contracts::update::status::StartupNotice>;
    /// Tell the coordinator, once, how the SugarCubes page failed during this start.
    fn report_page_failure(
        &mut self,
        failure: sugarsubstitute_supervision::lifecycle_protocol::PageFailure,
    ) -> BackendResult<()>;
    /// A writer that exports the open update report, whose plain text is `text`, to the file
    /// the user chooses; `None` hides Export and keeps Copy.
    fn update_report_export(
        &mut self,
        text: String,
    ) -> Option<Box<dyn sugarsubstitute_desktop::report::surface::UpdateExport>>;
    /// Tell the coordinator this close restarts the application so a prepared update applies.
    /// Called once, after the close save's outcome exists and before `request_shutdown`.
    fn request_restart_to_apply(&mut self) -> BackendResult<()>;
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
