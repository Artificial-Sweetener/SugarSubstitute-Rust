// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route the actual authenticated desktop pipe through the existing native owners.
#[path = "parent_lifecycle/acquisition.rs"]
mod acquisition;
#[path = "parent_lifecycle/activation.rs"]
mod activation;
#[path = "parent_lifecycle/activity.rs"]
mod activity;
#[path = "parent_lifecycle/attempt.rs"]
mod attempt;
#[cfg(test)]
pub(in crate::production) use attempt::SettledAttemptFailure;
#[path = "parent_lifecycle/backend.rs"]
mod backend;
#[path = "parent_lifecycle/causes.rs"]
pub(in crate::production) mod causes;
#[path = "parent_lifecycle/deadlines.rs"]
mod deadlines;
#[path = "parent_lifecycle/expiry.rs"]
mod expiry;
#[path = "parent_lifecycle/folders.rs"]
mod folders;
#[path = "parent_lifecycle/frontend_refresh.rs"]
mod frontend_refresh;
#[path = "parent_lifecycle/history.rs"]
mod history;
#[path = "parent_lifecycle/main.rs"]
mod main;
#[path = "parent_lifecycle/native_retirement.rs"]
mod native_retirement;
#[path = "parent_lifecycle/normal_close.rs"]
mod normal_close;
#[path = "parent_lifecycle/readiness.rs"]
mod readiness;
pub(crate) use normal_close::DesktopCloseRequest;
#[path = "parent_lifecycle/output.rs"]
mod output;
#[path = "parent_lifecycle/output_wait.rs"]
mod output_wait;
#[path = "parent_lifecycle/profile_observation.rs"]
mod profile_observation;
#[path = "parent_lifecycle/receive.rs"]
mod receive;
#[path = "parent_lifecycle/recovery.rs"]
pub(in crate::production) mod recovery;
#[path = "parent_lifecycle/replies.rs"]
mod replies;
#[path = "parent_lifecycle/requests.rs"]
mod requests;
#[path = "parent_lifecycle/retirement.rs"]
mod retirement;
#[path = "parent_lifecycle/retirement_trace.rs"]
mod retirement_trace;
#[cfg(test)]
#[path = "parent_lifecycle/tests.rs"]
mod tests;
#[path = "parent_lifecycle/transport.rs"]
mod transport;
#[path = "parent_lifecycle/types.rs"]
mod types;
#[path = "parent_lifecycle/watchdog.rs"]
mod watchdog;
#[path = "parent_lifecycle/write_receipt.rs"]
mod write_receipt;

use folders::Folders;
use history::Requests;
pub(crate) use retirement::RetiredRegistration;
use std::error::Error;
use sugarsubstitute_application::supervision::{MainAuthorizationSession, UiWatchdog};
use sugarsubstitute_contracts::{
    configuration::ProfileId,
    execution::{CompletionBatch, JobCompletion, JobSpec, Moment, ScopeId, TerminalKind},
    lifecycle::{ClientBinding, ConnectionId, LeaseKey, RunKey, ShutdownBudget},
    startup::LaunchId,
};
use sugarsubstitute_execution::windows_lifecycle_pipe::{
    LifecyclePipeDeadline, LifecyclePipeFailure, LifecyclePipeReport, ReceivedLifecycleFrame,
    VerifiedLifecyclePeer,
};
use sugarsubstitute_supervision::profiles::{BrowserFamilyProof, EnvironmentProfileProof};
use types::{ParentActivityFailureOrigin, PendingWrite, ProfilePending, Reply};
pub(crate) use types::{ParentError, ParentResources};

type Cause = Box<dyn Error + Send + Sync>;
type Result<T> = std::result::Result<T, Cause>;

/// Hold only original transport/session facts; construction authenticates nothing.
/// Data, original errors and guards precede their outside allocation claim in Drop.
pub(crate) struct ParentLifecycle {
    run: RunKey,
    launch: LaunchId,
    profile: ProfileId,
    connection: ConnectionId,
    original: LifecyclePipeDeadline,
    peer: Option<VerifiedLifecyclePeer>,
    prepared: bool,
    connected: bool,
    hello: bool,
    welcome: bool,
    registered: bool,
    closed: bool,
    latest_operation: u64,
    read: Option<u64>,
    write: Option<PendingWrite>,
    requests: Requests,
    acquisition: Option<acquisition::Acquisition>,
    folders: Folders,
    main: MainAuthorizationSession,
    watchdog: Option<UiWatchdog>,
    activity_revision: u64,
    lease: Option<LeaseKey>,
    recovery: recovery::Recovery,
    frontend_refresh: Option<frontend_refresh::FrontendRefresh>,
    backend_failed_resource: Option<sugarsubstitute_contracts::lifecycle::ResourceKey>,
    backend_failure_seen: bool,
    _attempt_failure: Option<Cause>,
    attempt_cleanup: Option<attempt::AttemptSettlement>,
    output: Option<output::Delivery>,
    next_delivery: u64,
    output_lease: Option<LeaseKey>,
    acknowledged_output: Option<(LeaseKey, std::num::NonZeroU64)>,
    challenge: Option<watchdog::Challenge>,
    retired_challenges: [Option<watchdog::Challenge>; history::SLOTS],
    profile_pending: Option<ProfilePending>,
    environment: Option<EnvironmentProfileProof>,
    browser: Option<BrowserFamilyProof>,
    exit: Option<sugarsubstitute_application::supervision::DesktopExitTransaction>,
    cleanup: Option<ShutdownBudget>,
    main_retirement: Option<(usize, sugarsubstitute_contracts::startup::MainIdentity)>,
    expected_terminal: Option<(ScopeId, JobSpec, TerminalKind)>,
    terminal: Option<JobCompletion<LifecyclePipeReport, LifecyclePipeFailure>>,
    terminal_accepted: bool,
    checked_retirement:
        Option<sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeRetirement>,
    terminal_failure_reported: bool,
    activity_failure_origin: ParentActivityFailureOrigin,
    rejected_frame: Option<ReceivedLifecycleFrame>,
    acquisition_receive: Option<(u64, ReceivedLifecycleFrame)>,
    output_receive_until: Option<Moment>,
    failure: Option<Cause>,
    secondary: [Option<Cause>; 8],
    omitted_causes: u64,
    forwarded: Option<sugarsubstitute_contracts::lifecycle::ResourceTransition>,
    disconnect_attempted: bool,
    close_after_reply: Option<usize>,
    backend_causes: Option<causes::BackendCauses>,
}

impl ParentLifecycle {
    /// Bound actor storage, copied targets, fixed history and first local error.
    /// The caller claims this distinct partition before construction from the same
    /// actual projection completion that retains the original launch/configuration.
    pub(crate) const fn storage_bytes() -> u64 {
        2 * 1024 * 1024
    }

    /// Fixed actor and eight original output buffers inside the unchanged Parent partition.
    /// The receiving composition separately preflights dynamic policy/transition/input peaks before effects.
    pub(crate) const fn fixed_storage_bytes() -> u64 {
        (std::mem::size_of::<Self>() + history::SLOTS * 32768) as u64
            + causes::BackendCauses::heap_bytes()
            + crate::production::coordinator_backend::CoordinatorBackend::FRONTEND_RELOAD_TARGET_BYTES
    }

    /// Preserve original run/connection/QPC facts after actual pipe preparation.
    /// This constructor performs no IO, provider call or authentication.
    pub(crate) fn new(
        run: RunKey,
        launch: LaunchId,
        profile: ProfileId,
        connection: ConnectionId,
        original: LifecyclePipeDeadline,
        causes: &mut Option<causes::BackendCauses>,
    ) -> std::result::Result<Self, ParentError> {
        if causes.as_ref().is_none_or(|owner| owner.storage_bytes() != Self::storage_bytes())
            || Self::fixed_storage_bytes() > Self::storage_bytes()
        {
            return Err(ParentError::Storage);
        }
        Ok(Self {
            run,
            launch,
            profile,
            connection,
            original,
            peer: None,
            prepared: false,
            connected: false,
            hello: false,
            welcome: false,
            registered: false,
            closed: false,
            latest_operation: 0,
            read: None,
            write: None,
            requests: Requests::new(),
            acquisition: None,
            folders: Folders::new(),
            main: MainAuthorizationSession::new(run),
            watchdog: None,
            activity_revision: 0,
            lease: None,
            recovery: recovery::Recovery::default(),
            frontend_refresh: None,
            backend_failed_resource: None,
            backend_failure_seen: false,
            _attempt_failure: None,
            attempt_cleanup: None,
            output: None,
            next_delivery: 0,
            output_lease: None,
            acknowledged_output: None,
            challenge: None,
            retired_challenges: [None; history::SLOTS],
            profile_pending: None,
            environment: None,
            browser: None,
            exit: None,
            cleanup: None,
            main_retirement: None,
            expected_terminal: None,
            terminal: None,
            terminal_accepted: false,
            checked_retirement: None,
            terminal_failure_reported: false,
            activity_failure_origin: ParentActivityFailureOrigin::Coordinator,
            rejected_frame: None,
            acquisition_receive: None,
            output_receive_until: None,
            failure: None,
            secondary: [const { None }; 8],
            omitted_causes: 0,
            forwarded: None,
            disconnect_attempted: false,
            close_after_reply: None,
            backend_causes: Some(causes.take().ok_or(ParentError::Storage)?),
        })
    }

    /// Retain the original finite cleanup transaction; a repeated call cannot renew it.
    pub(crate) fn request_cleanup(
        &mut self,
        budget: ShutdownBudget,
        resources: &mut ParentResources<'_, '_>,
    ) -> std::result::Result<(), ParentError> {
        self.validate_cleanup_budget(budget)?;
        self.cleanup = Some(budget);
        if !self.closed {
            tracing::info!(operation="parent_owned_cleanup_requested",run=?self.run,
                started=?budget.started,graceful_until=?budget.graceful_until,settle_until=?budget.settle_until,
                "Original role cleanup requested; physical retirement remains independently owned");
        }
        self.closed = true;
        // Retain the first semantic cause while releasing its immutable input loan
        // so the actual pipe worker can finish checked native retirement.
        self.rejected_frame = None;
        self.acquisition_receive = None;
        self.output_receive_until = None;
        self.output = None;
        if let Some(write) = &self.write {
            let _fact = write.admission.revoke();
        }
        resources.session.control.close();
        if let Some(identity) = self.main.snapshot().identity
            && self.main.stage()
                != sugarsubstitute_application::supervision::MainAuthorizationStage::Retired
        {
            if let Err(cause) = self.main.retire(identity) {
                self.retain_failure(cause.into());
            }
            if let Err(cause) = resources.profile.retire_main_attempt(identity) {
                self.retain_failure(cause.into());
            }
            self.environment = None;
            self.browser = None;
        }
        if let Err(cause) = self.finish_disconnect(resources) {
            self.retain_failure(cause);
            return Err(ParentError::OwnerFailed);
        }
        Ok(())
    }

    /// Actual pipe retirement is independent of desktop/profile/backend settlement.
    pub(crate) fn settled(&self) -> bool {
        self.native_retired()
            && self.profile_pending.is_none()
            && self.frontend_refresh.as_ref().is_none_or(|refresh| refresh.retired())
            && self.forwarded.is_none()
            && (!self.registered || self.disconnect_attempted)
    }

    /// Report only this exact pipe's checked native retirement and released loans.
    /// An outcome, notice, cancellation request or process exit without the checked proof is insufficient.
    pub(crate) fn native_retired(&self) -> bool {
        self.terminal_accepted
            && self.checked_retirement.as_ref().is_some_and(|proof| {
                proof.run() == Some(self.run)
                    && self.terminal.as_ref().is_some_and(|terminal| {
                        proof.scope_id() == terminal.scope_id && proof.spec() == terminal.spec
                    })
            })
            && self.read.is_none()
            && self.write.is_none()
            && self.rejected_frame.is_none()
            && self.acquisition_receive.is_none()
            && self.output.is_none()
    }

    /// Borrow the first original owner failure without exposing sensitive payloads.
    pub(crate) fn failure(&self) -> Option<&(dyn Error + Send + Sync + 'static)> {
        self.failure.as_deref()
    }

    /// Report a whole actual producer return still owned locally, before Root captures its metadata.
    pub(in crate::production) fn pending_backend_delivery(&self) -> bool {
        self.forwarded.is_some()
    }

    /// Block fresh shared backend commands while this actual native binding handoff is unconsumed.
    /// Completion adoption, checked cleanup and the authenticated current OutputPull remain admissible.
    pub(crate) fn pending_backend_handoff(&self) -> bool {
        !self.closed && self.recovery.handoff.is_some()
    }

    /// Borrow the actual settled cause for owner custody assertions.
    #[cfg(test)]
    pub(crate) fn settled_attempt_failure(&self) -> Option<&SettledAttemptFailure> {
        self.backend_causes.as_ref()?.first()
    }
    /// Observe the first failed backend request's actual reply association.
    /// Absence grants no inference about backend records or readiness. The caller
    /// borrows that exact backend owner's retained failure/cleanup cause separately.
    pub(crate) const fn backend_failure_resource(
        &self,
    ) -> Option<sugarsubstitute_contracts::lifecycle::ResourceKey> {
        self.backend_failed_resource
    }

    fn retain_failure(&mut self, cause: Cause) {
        if self.failure.is_none() {
            self.failure = Some(cause);
        } else if let Some(slot) = self.secondary.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(cause);
        } else {
            self.omitted_causes = self.omitted_causes.saturating_add(1);
        }
    }

    /// Transfer foreign-run replies emitted by the actual shared backend owner.
    /// Native effects already ran there; the caller routes replies only, once.
    pub(crate) fn take_forwarded_transition(
        &mut self,
    ) -> Option<sugarsubstitute_contracts::lifecycle::ResourceTransition> {
        self.forwarded.take()
    }

    /// Return only the retained actual exit transaction's original cleanup interval.
    pub(crate) const fn shutdown(&self) -> Option<ShutdownBudget> {
        self.cleanup
    }

    /// Return cached original boundaries for the composing native wait pump.
    pub(crate) fn deadlines(&self) -> impl Iterator<Item = Moment> + '_ {
        let active = !self.settled();
        let ui = self.watchdog.as_ref().and_then(|watchdog| {
            let snapshot = watchdog.snapshot();
            snapshot.suspicion_at.or(snapshot.next_challenge_at)
        });
        [
            self.cleanup.map(|budget| budget.graceful_until),
            self.cleanup.map(|budget| budget.settle_until),
            ui,
            self.profile_pending.as_ref().map(|pending| pending.original),
            self.main_deadline(),
            self.requests.next_deadline(),
            self.output_receive_until,
            self.frontend_refresh.as_ref().and_then(|refresh| refresh.deadline()),
        ]
        .into_iter()
        .flatten()
        .filter(move |_| active)
    }

    /// Observe only the retained native watchdog's actual dispatched suspicion.
    pub(crate) fn ui_suspected(&self) -> bool {
        self.watchdog.as_ref().is_some_and(|watchdog| watchdog.snapshot().suspicious)
    }

    fn binding(&self) -> std::result::Result<ClientBinding, ParentError> {
        if self.peer.is_none() || !self.registered || self.closed {
            return Err(ParentError::WrongPhase);
        }
        Ok(ClientBinding { run: self.run, connection: self.connection })
    }
}
