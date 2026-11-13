// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain real entry, authority, child and execution lifetimes through native pumping.
#[path = "entry_runner/forward_recovery.rs"]
mod forward_recovery;
#[path = "entry_runner/initialization.rs"]
mod initialization;
#[path = "entry_runner/lifecycle.rs"]
mod lifecycle;
#[path = "entry_runner/normal_idle.rs"]
mod normal_idle;
#[path = "entry_runner/post_exit.rs"]
mod post_exit;
#[path = "entry_runner/presentation.rs"]
pub(super) mod presentation;
#[path = "entry_runner/relaunch.rs"]
mod relaunch;
#[path = "entry_runner/routing.rs"]
mod routing;
#[cfg(test)]
#[path = "entry_runner/tests.rs"]
pub(crate) mod tests;
#[path = "entry_runner/updates.rs"]
mod updates;
use super::{
    coordinator_authority::CoordinatorAuthority, dispatch::ProductionCommand,
    entry::ProductionEntryIntake, entry_bootstrap::EntryBootstrap,
};
use crate::job_sequence::JobSequence;
use crate::scope_sequence::ScopeSequence;
use std::{
    error::Error,
    fmt,
    num::NonZeroUsize,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::{
    coordinator_entry::EntrySetupBudget,
    execution::{CompletionBatch, CompletionWake, ExecutionPort, JobCompletion, Scope, ScopeSpec},
    lifecycle::ShutdownBudget,
};
use sugarsubstitute_desktop::{
    HeadlessLifetime, HeadlessStep, StartupPreparationStep, run_headless,
};
use sugarsubstitute_execution::{
    AuthenticatedEntryLaunch, CoordinatorEntryClientSession, EntryTransportError,
    EntryTransportReport, ExecutionClient, ExecutionOwner, MonotonicClock,
};
use sugarsubstitute_supervision::coordinator_entry::CoordinatorEntryOwner;

/// Borrow actual retained owner capabilities on the native composition thread.
/// Matching keys, cached observations and presentation data cannot create these owners.
pub(crate) struct EntryResources<'a> {
    pub(crate) client: &'a ExecutionClient,
    pub(crate) finite: &'a Scope,
    pub(crate) native: &'a Scope,
    pub(crate) cleanup: &'a Scope,
    pub(crate) jobs: &'a JobSequence,
    pub(crate) scopes: &'a ScopeSequence,
    pub(crate) entry: &'a mut CoordinatorEntryOwner,
    pub(crate) authority: &'a mut CoordinatorAuthority,
    pub(crate) intake: &'a mut ProductionEntryIntake,
}

/// Real parent-child constructor boundary; the executable supplies its actual adapter.
/// Every method runs on the native thread and returns promptly. Partial child
/// admissions, authenticated loans and original causes stay in this adapter until
/// checked retirement. No method may fabricate a Run, readiness or family settlement.
pub(crate) trait EntryActivation {
    /// Borrow the already authenticated locale; absence uses canonical fallback without reading settings.
    fn presentation_locale(&self) -> Option<sugarsubstitute_contracts::configuration::LocaleId>;
    /// Consume a genuinely authenticated source into the existing child admission.
    /// On failure retain partial admitted custody and its original cause in this owner.
    /// Unexecuted rejection leaves this identical option unchanged. An admitted
    /// transfer takes it only after the adapter retains the actual source/pins.
    fn enter_activation(
        &mut self,
        request: &mut Option<AuthenticatedEntryLaunch>,
        resources: EntryResources<'_>,
    ) -> Result<(), Box<dyn Error>>;
    /// Observe the same completion batch supplied to every composing owner.
    fn consume(
        &mut self,
        batch: &CompletionBatch,
        resources: EntryResources<'_>,
    ) -> Result<HeadlessStep, Box<dyn Error>>;
    /// Close child admission before native sessions and authority retirement.
    fn request_close(&mut self, budget: Option<ShutdownBudget>) -> Result<(), Box<dyn Error>>;
    /// Borrow its already-issued same-executor shutdown budget, without sampling.
    fn shutdown_budget(&self) -> Option<ShutdownBudget>;
    /// Borrow actual deliberate close intent only when current role and diagnostic custody permit it.
    /// This samples no clock and proves neither intake quiescence nor native settlement.
    fn normal_close_requested(&self) -> bool;
    /// Offer the coordinator's update status at `revision` to every live desktop child; each
    /// keeps it when newer than its own, so the runner may offer it every turn.
    /// The child also learns the backend's workflow queue from its parent.
    /// # Errors
    /// Returns the status's encoding failure.
    fn publish_update_status(
        &mut self,
        revision: u64,
        status: &sugarsubstitute_contracts::update::status::UpdateStatus,
    ) -> Result<(), Box<dyn Error>>;
    /// Whether a desktop child that asked to restart so an update applies has since completed
    /// a normal close. A crashed or failed child never counts.
    fn restart_to_apply_requested(&self) -> bool;
    /// Take, once, how a live desktop child said the SugarCubes page failed during its start.
    fn take_page_failure(
        &mut self,
    ) -> Option<sugarsubstitute_supervision::lifecycle_protocol::PageFailure>;
    /// Borrow its original retained fatal failure without separating its custody.
    /// An exposed source closes runner admission even when the last turn returned Active.
    fn failure_source(&self) -> Option<&(dyn Error + 'static)>;
    /// Which process the exposed failure belongs to: the backend when it came from a backend
    /// resource, otherwise the host itself. Decides an unaccepted update trial's verdict.
    fn failed_process(&self) -> sugarsubstitute_contracts::installation::FailedProcess;
    /// How the desktop attributed its own exit in its authenticated exit intent, once it sent
    /// one: a deliberate close (including a close for a session end) or a startup failure.
    /// Absent while no desktop has announced its exit.
    fn desktop_close_intent(
        &self,
    ) -> Option<sugarsubstitute_contracts::lifecycle::CompletionIntent>;
    /// Return its actual next native-thread wake without introducing timers.
    fn next_deadline(&self) -> Option<Instant>;
    /// Observe actual child/process/transport settlement, retaining uncertainty.
    fn poll_close(
        &mut self,
        batch: &CompletionBatch,
        resources: Option<EntryResources<'_>>,
        budget: ShutdownBudget,
    ) -> Result<bool, Box<dyn Error>>;
    /// Finish only after real settlement and execution joins on this same thread.
    fn finish(&mut self) -> Result<(), Box<dyn Error>>;
}

/// Drive production with a uniquely owned heap child, keeping its value outside the persistent pump frame.
/// # Errors
/// Keeps the entire failed runner, original sources and scopes owned after pump failure.
pub(crate) fn run<C: EntryActivation + 'static>(
    command: ProductionCommand,
    child: impl FnOnce(&ProductionCommand) -> Result<Box<C>, Box<dyn Error>>,
    diagnostics: crate::diagnostics::DiagnosticHealth,
) -> Result<(), Box<dyn Error>> {
    let executable = std::env::current_exe()?;
    let child = child(&command)?;
    drive(boxed_lifetime(command, executable, child, diagnostics))
}

/// Pump a constructed lifetime to settlement, keeping the failed runner and its causes owned.
/// # Errors
/// Returns the retained original failure after the headless owner's checked shutdown.
fn drive<C: EntryActivation + 'static>(
    mut lifetime: Box<EntryRunner<C>>,
) -> Result<(), Box<dyn Error>> {
    match run_headless(lifetime.as_mut()) {
        Ok(()) => Ok(()),
        Err(pump) => Err(retain_failure(Box::new(pump), lifetime)),
    }
}

/// Run the coordinator without a native splash; the Desktop owns its entire UI lifetime.
/// # Errors
/// Reports the retained original failure after the headless owner's checked shutdown.
pub(crate) fn run_application<C: EntryActivation + 'static>(
    command: ProductionCommand,
    child: impl FnOnce(&ProductionCommand) -> Result<Box<C>, Box<dyn Error>>,
    diagnostics: crate::diagnostics::DiagnosticHealth,
) -> Result<(), Box<dyn Error>> {
    let cause = match run(command, child, diagnostics) {
        Ok(()) => return Ok(()),
        Err(cause) => cause,
    };
    if let Some(failure) = cause.downcast_ref::<RunFailure<C>>()
        && failure.lifetime.finished
        && let Some(request) =
            sugarsubstitute_supervision::profiles::ProfileInUse::request(cause.as_ref())
    {
        match super::profile_conflict::resolve(request, presentation::locale::<C>(cause.as_ref())) {
            Ok(_) => return Ok(()),
            Err(secondary) => {
                tracing::error!(operation="profile_conflict_presentation", cause=?FailureDisplay(secondary.as_ref()),
                    "Conflict resolution failed; retaining the original startup failure");
            }
        }
    }
    Err(super::entry_failure::report_failure::<C, _>(
        cause,
        sugarsubstitute_desktop::show_startup_failure,
    ))
}

/// Construct the runner from its original heap child without moving that child value onto the pump stack.
#[inline(never)]
fn boxed_lifetime<C: EntryActivation>(
    command: ProductionCommand,
    executable: PathBuf,
    child: Box<C>,
    diagnostics: crate::diagnostics::DiagnosticHealth,
) -> Box<EntryRunner<C>> {
    Box::new(EntryRunner::new(command, executable, child, diagnostics))
}

/// Keep the original entry custody outside the native pump's stack frame.
#[cold]
#[inline(never)]
pub(in crate::production) fn retain_failure<C: EntryActivation + 'static>(
    pump: Box<dyn Error>,
    lifetime: Box<EntryRunner<C>>,
) -> Box<dyn Error> {
    let failure = RunFailure { pump, lifetime };
    tracing::error!(operation="production_entry_failed",cause_chain=%crate::diagnostics::CauseChain(failure.primary()),
        "Production entry failed with retained startup and cleanup causes");
    Box::new(failure)
}
struct RunFailure<C: EntryActivation> {
    pump: Box<dyn Error>,
    lifetime: Box<EntryRunner<C>>,
}
/// A completed user close returns its original failure without reopening a modal
/// that would keep process-owned claims alive until the user dismisses it.
pub(super) fn is_normal_shutdown_failure<C: EntryActivation + 'static>(
    cause: &(dyn Error + 'static),
) -> bool {
    let mut current = Some(cause);
    for _ in 0..16 {
        let Some(cause) = current else { break };
        if let Some(failure) = cause.downcast_ref::<RunFailure<C>>() {
            return failure.lifetime.normal_shutdown;
        }
        current = cause.source();
    }
    false
}
impl<C: EntryActivation> RunFailure<C> {
    /// Borrow the original application cause, including failure before owner initialization.
    fn primary(&self) -> &(dyn Error + 'static) {
        self.lifetime.failure_source().unwrap_or(self.pump.as_ref())
    }
}
/// Format at most sixteen borrowed causes, preserving configuration-source redaction.
pub(super) struct FailureDisplay<'a>(pub(super) &'a (dyn Error + 'static));
impl fmt::Debug for FailureDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut cause = Some(self.0);
        for depth in 0..16 {
            let Some(current) = cause else { return Ok(()) };
            if depth != 0 {
                f.write_str("; caused by: ")?;
            }
            if let Some(transport) = current.downcast_ref::<EntryTransportError>()
                && matches!(
                    transport,
                    EntryTransportError::Closed
                        | EntryTransportError::Busy
                        | EntryTransportError::InvalidOperation
                        | EntryTransportError::InvalidDeadline
                        | EntryTransportError::InvalidPhase
                        | EntryTransportError::Authentication
                        | EntryTransportError::Malformed
                        | EntryTransportError::Capacity
                )
            {
                fmt::Debug::fmt(transport, f)?;
            } else {
                fmt::Display::fmt(current, f)?;
            }
            if current
                .downcast_ref::<sugarsubstitute_configuration::startup::StartupConfigurationError>()
                .is_some()
            {
                return Ok(());
            }
            cause = current.source();
        }
        if cause.is_some() {
            f.write_str("; further causes retained")?;
        }
        Ok(())
    }
}
impl<C: EntryActivation> fmt::Debug for RunFailure<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProductionEntryFailure")
            .field("primary", &FailureDisplay(self.primary()))
            .field(
                "bootstrap_post_control",
                &self.lifetime.bootstrap.as_ref().and_then(EntryBootstrap::post_control),
            )
            .field(
                "bootstrap_cleanup",
                &self
                    .lifetime
                    .bootstrap
                    .as_ref()
                    .and_then(EntryBootstrap::cleanup_failure)
                    .map(FailureDisplay),
            )
            .field(
                "authority_cleanup",
                &self
                    .lifetime
                    .authority
                    .as_ref()
                    .and_then(CoordinatorAuthority::cleanup_failure)
                    .map(FailureDisplay),
            )
            .finish_non_exhaustive()
    }
}
impl<C: EntryActivation> fmt::Display for RunFailure<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Production entry failed")
    }
}
impl<C: EntryActivation + 'static> Error for RunFailure<C> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.primary())
    }
}

#[derive(Clone, Copy)]
enum FailureOrigin {
    Bootstrap,
    Authority,
    Owned,
    Child,
    Transport(usize),
}

/// Keep the original heap child ahead of scopes and the sole execution join owner.
pub(crate) struct EntryRunner<C: EntryActivation> {
    diagnostics: crate::diagnostics::DiagnosticHealth,
    child: Box<C>,
    pending_activation: Option<AuthenticatedEntryLaunch>,
    authority: Option<CoordinatorAuthority>,
    intake: Option<ProductionEntryIntake>,
    bootstrap: Option<EntryBootstrap>,
    updater: Option<crate::updates::parent::Parent>,
    update_schedule: Option<crate::updates::schedule::CheckSchedule>,
    /// Native wake for the next due update check; the check itself runs on the executor.
    update_wake: Option<Instant>,
    /// What the update control shows, from attempt results and the selection record.
    update_status: crate::updates::status::StatusSource,
    /// Scope of the selection-record reads behind `update_status`.
    update_status_scope: Option<Scope>,
    post_exit: Option<post_exit::Deadline>,
    /// The one restart-to-apply relaunch, after a normal close the child asked for.
    relaunch: relaunch::Relaunch,
    command: Option<ProductionCommand>,
    executable: PathBuf,
    installation: PathBuf,
    jobs: JobSequence,
    scopes: ScopeSequence,
    finite: Option<Scope>,
    source: Option<Scope>,
    native: Option<Scope>,
    cleanup: Option<Scope>,
    client: Option<ExecutionClient>,
    execution: Option<ExecutionOwner>,
    clock: Arc<dyn sugarsubstitute_contracts::execution::Clock>,
    /// Samples the native monotonic instant that anchors every native-thread wake. Production
    /// reads the real clock; runner tests freeze it so elapsed time never expires a cutoff.
    native_now: fn() -> Instant,
    original: Option<EntrySetupBudget>,
    setup_deadline: Option<Instant>,
    shutdown: Option<ShutdownBudget>,
    cleanup_deadline: Option<Instant>,
    failure: Option<Box<dyn Error>>,
    first_origin: Option<FailureOrigin>,
    failure_reported: bool,
    secondary: [Option<Box<dyn Error>>; 8],
    omitted: u64,
    authority_started: bool,
    initial_completed: bool,
    normal_shutdown: bool,
    closing: bool,
    child_closed: bool,
    intake_settled: bool,
    intake_ready: bool,
    forward: Option<CoordinatorEntryClientSession>,
    transport_completions: [Option<JobCompletion<EntryTransportReport, EntryTransportError>>; 3],
    deadline_reported: bool,
    execution_stopping: bool,
    shutdown_diagnostic: Option<Instant>,
    finished: bool,
}
impl<C: EntryActivation> EntryRunner<C> {
    pub(crate) fn new(
        command: ProductionCommand,
        executable: PathBuf,
        child: Box<C>,
        diagnostics: crate::diagnostics::DiagnosticHealth,
    ) -> Self {
        let installation = command.installation.clone();
        Self {
            diagnostics,
            child,
            pending_activation: None,
            authority: None,
            intake: None,
            bootstrap: None,
            updater: None,
            update_schedule: None,
            update_wake: None,
            update_status: crate::updates::status::StatusSource::new(),
            update_status_scope: None,
            post_exit: None,
            relaunch: relaunch::Relaunch::Idle,
            command: Some(command),
            executable,
            installation,
            jobs: JobSequence::new(),
            scopes: ScopeSequence::new(),
            finite: None,
            source: None,
            native: None,
            cleanup: None,
            client: None,
            execution: None,
            clock: MonotonicClock::new(),
            native_now: Instant::now,
            original: None,
            setup_deadline: None,
            shutdown: None,
            cleanup_deadline: None,
            failure: None,
            first_origin: None,
            failure_reported: false,
            secondary: std::array::from_fn(|_| None),
            omitted: 0,
            authority_started: false,
            initial_completed: false,
            normal_shutdown: false,
            closing: false,
            child_closed: false,
            intake_settled: false,
            intake_ready: false,
            forward: None,
            transport_completions: std::array::from_fn(|_| None),
            deadline_reported: false,
            execution_stopping: false,
            shutdown_diagnostic: None,
            finished: false,
        }
    }
    fn retain(&mut self, cause: Box<dyn Error>) {
        let origin = *self.first_origin.get_or_insert(FailureOrigin::Owned);
        if self.failure.is_none() {
            let process = match origin {
                FailureOrigin::Child => self.child.failed_process(),
                _ => sugarsubstitute_contracts::installation::FailedProcess::Host,
            };
            if let Some(authority) = &mut self.authority {
                authority.observe_trial_failure(process);
            }
            self.failure = Some(cause);
            if let Some(primary) = self.failure_source() {
                tracing::error!(operation="production_ownership_failed",cause_chain=%crate::diagnostics::CauseChain(primary),
                    "Production admission closed with its original retained cause");
            }
        } else if let Some(slot) = self.secondary.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(cause);
        } else {
            self.omitted = self.omitted.saturating_add(1);
        }
    }
    fn close_on_failure(&mut self, cause: Box<dyn Error>) {
        self.retain(cause);
        if let Err(cause) = self.request_shutdown() {
            self.retain(cause);
        }
    }
    /// Retain a newly exposed fatal child source once, preserving any earlier primary owner.
    /// The child keeps its exact cause; this marker only starts the existing failure path.
    fn observe_retained_child_failure(&mut self) -> bool {
        let failed = self.child.failure_source().is_some();
        if failed && self.first_origin.is_none() {
            self.first_origin = Some(FailureOrigin::Child);
            self.retain("Actual child retained a fatal failure after its turn".into());
        }
        failed
    }
    fn failure_source(&self) -> Option<&(dyn Error + 'static)> {
        match self.first_origin {
            Some(FailureOrigin::Bootstrap) => {
                self.bootstrap.as_ref().and_then(EntryBootstrap::failure_source)
            }
            Some(FailureOrigin::Authority) => {
                self.authority.as_ref().and_then(CoordinatorAuthority::failure_source)
            }
            Some(FailureOrigin::Child) => self.child.failure_source(),
            Some(FailureOrigin::Transport(index)) => {
                self.transport_completions.get(index).and_then(Option::as_ref).and_then(
                    |completion| match &completion.outcome {
                        sugarsubstitute_contracts::execution::JobOutcome::Failure(cause) => {
                            Some(cause as &(dyn Error + 'static))
                        }
                        _ => None,
                    },
                )
            }
            Some(FailureOrigin::Owned) | None => None,
        }
        .or(self.failure.as_deref())
    }
    fn client(&self) -> Result<&ExecutionClient, Box<dyn Error>> {
        self.client.as_ref().ok_or_else(|| "Production execution client missing".into())
    }
    fn drain(&mut self, max: NonZeroUsize) -> Result<HeadlessStep, Box<dyn Error>> {
        if max.get() > 32 {
            return Err("Production completion budget exceeds32".into());
        }
        let batch = match self.client()?.drain_completions(max) {
            Ok(batch) => batch,
            Err(cause) => {
                self.close_on_failure(Box::new(cause));
                return Err(
                    "Production completion drain failed with its original retained cause".into()
                );
            }
        };
        self.consume_batch(&batch)
    }
    fn consume_batch(&mut self, batch: &CompletionBatch) -> Result<HeadlessStep, Box<dyn Error>> {
        if (self.original.is_some() || self.closing)
            && let Err(cause) = self.route(batch)
        {
            self.close_on_failure(cause);
            return Err(
                "Production completion routing failed with its original retained cause".into()
            );
        }
        if !self.closing
            && let Err(cause) = self.diagnostics.check()
        {
            self.close_on_failure(Box::new(cause));
            return Err("Diagnostic output failed with its original retained cause".into());
        }
        Ok(if self.closing { HeadlessStep::Shutdown } else { HeadlessStep::Active })
    }
}
