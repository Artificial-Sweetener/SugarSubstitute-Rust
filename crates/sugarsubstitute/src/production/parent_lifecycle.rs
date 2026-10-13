// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route the actual authenticated desktop pipe through the existing native owners.
#[path = "parent_lifecycle/backend.rs"]
mod backend;
#[path = "parent_lifecycle/folders.rs"]
mod folders;
#[path = "parent_lifecycle/history.rs"]
mod history;
#[path = "parent_lifecycle/main.rs"]
mod main;
#[path = "parent_lifecycle/output.rs"]
mod output;
#[path = "parent_lifecycle/replies.rs"]
mod replies;
#[path = "parent_lifecycle/requests.rs"]
mod requests;
#[path = "parent_lifecycle/retirement.rs"]
mod retirement;
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
    execution::{
        CompletionBatch, JobCompletion, JobSpec, MemoryClaim, Moment, ScopeId, TerminalKind,
    },
    lifecycle::{ClientBinding, ConnectionId, LeaseKey, RunKey, ShutdownBudget},
    startup::LaunchId,
};
use sugarsubstitute_execution::windows_lifecycle_pipe::{
    LifecyclePipeDeadline, LifecyclePipeFailure, LifecyclePipeReport, ReceivedLifecycleFrame,
    VerifiedLifecyclePeer,
};
use sugarsubstitute_supervision::profiles::{BrowserFamilyProof, EnvironmentProfileProof};
pub(crate) use types::{ParentError, ParentResources};
use types::{PendingWrite, ProfilePending, Reply};

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
    folders: Folders,
    main: MainAuthorizationSession,
    watchdog: Option<UiWatchdog>,
    activity_revision: u64,
    lease: Option<LeaseKey>,
    backend_failed_resource: Option<sugarsubstitute_contracts::lifecycle::ResourceKey>,
    backend_failure_seen: bool,
    output: Option<output::Delivery>,
    next_delivery: u64,
    challenge: Option<(
        sugarsubstitute_contracts::lifecycle::UiChallengeKey,
        sugarsubstitute_supervision::lifecycle_protocol::WireHeader,
        Moment,
    )>,
    profile_pending: Option<ProfilePending>,
    environment: Option<EnvironmentProfileProof>,
    browser: Option<BrowserFamilyProof>,
    exit: Option<sugarsubstitute_application::supervision::DesktopExitTransaction>,
    cleanup: Option<ShutdownBudget>,
    main_retirement: Option<(usize, sugarsubstitute_contracts::startup::MainIdentity)>,
    expected_terminal: Option<(ScopeId, JobSpec, TerminalKind)>,
    terminal: Option<JobCompletion<LifecyclePipeReport, LifecyclePipeFailure>>,
    terminal_accepted: bool,
    rejected_frame: Option<ReceivedLifecycleFrame>,
    failure: Option<Cause>,
    secondary: [Option<Cause>; 8],
    omitted_causes: u64,
    forwarded: Option<sugarsubstitute_contracts::lifecycle::ResourceTransition>,
    disconnect_attempted: bool,
    close_after_reply: Option<usize>,
    _storage: MemoryClaim,
}

impl ParentLifecycle {
    /// Bound actor storage, copied targets, fixed history and first local error.
    /// The caller claims this distinct partition before construction from the same
    /// actual projection completion that retains the original launch/configuration.
    pub(crate) const fn storage_bytes() -> u64 {
        2 * 1024 * 1024
    }

    /// Preserve original run/connection/QPC facts after actual pipe preparation.
    /// This constructor performs no IO, provider call or authentication.
    pub(crate) fn new(
        run: RunKey,
        launch: LaunchId,
        profile: ProfileId,
        connection: ConnectionId,
        original: LifecyclePipeDeadline,
        storage: MemoryClaim,
    ) -> std::result::Result<Self, ParentError> {
        if storage.bytes() != Self::storage_bytes() {
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
            folders: Folders::new(),
            main: MainAuthorizationSession::new(run),
            watchdog: None,
            activity_revision: 0,
            lease: None,
            backend_failed_resource: None,
            backend_failure_seen: false,
            output: None,
            next_delivery: 0,
            challenge: None,
            profile_pending: None,
            environment: None,
            browser: None,
            exit: None,
            cleanup: None,
            main_retirement: None,
            expected_terminal: None,
            terminal: None,
            terminal_accepted: false,
            rejected_frame: None,
            failure: None,
            secondary: [const { None }; 8],
            omitted_causes: 0,
            forwarded: None,
            disconnect_attempted: false,
            close_after_reply: None,
            _storage: storage,
        })
    }

    /// Consume this pipe/profile's actual jobs from the one immutable global batch.
    /// Registry/claim consumption remains with the coordinator before this call.
    /// Every original frame and first failure remains owned on rejection.
    pub(crate) fn activity(
        &mut self,
        batch: &CompletionBatch,
        resources: &mut ParentResources<'_, '_>,
    ) -> std::result::Result<(), ParentError> {
        let result = self.advance(batch, resources);
        if let Err(cause) = result {
            self.retain_failure(cause);
            self.closed = true;
            resources.session.control.close();
            return Err(ParentError::OwnerFailed);
        }
        Ok(())
    }

    fn advance(
        &mut self,
        batch: &CompletionBatch,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<()> {
        self.latch_terminal(batch, resources)?;
        self.consume_profile(batch, resources)?;
        self.finish_main_retirement(resources)?;
        while let Some(observation) = resources.session.control.take_observation() {
            self.observe(observation, resources)?;
        }
        self.consume_terminal(resources)?;
        if self.closed {
            self.finish_disconnect(resources)?;
            return Ok(());
        }
        self.check_main_deadline(resources)?;
        self.send_next(resources)?;
        self.receive_next(resources)?;
        Ok(())
    }

    /// Retain the original finite cleanup transaction; a repeated call cannot renew it.
    pub(crate) fn request_cleanup(
        &mut self,
        budget: ShutdownBudget,
        resources: &mut ParentResources<'_, '_>,
    ) -> std::result::Result<(), ParentError> {
        if self.cleanup.is_some_and(|original| original != budget) {
            return Err(ParentError::ChangedDuplicate);
        }
        self.cleanup = Some(budget);
        self.closed = true;
        // Retain the first semantic cause while releasing its immutable input loan
        // so the actual pipe worker can finish checked native retirement.
        self.rejected_frame = None;
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
            && self.forwarded.is_none()
            && (!self.registered || self.disconnect_attempted)
    }

    /// Report only this exact pipe's checked native retirement and released loans.
    /// Failure, a notice, requested cancellation and process exit are insufficient.
    pub(crate) fn native_retired(&self) -> bool {
        self.terminal_accepted
            && self.terminal.as_ref().is_some_and(|terminal| {
                matches!(
                    terminal.outcome,
                    sugarsubstitute_contracts::execution::JobOutcome::Success(_)
                        | sugarsubstitute_contracts::execution::JobOutcome::Cancelled
                )
            })
            && self.read.is_none()
            && self.write.is_none()
            && self.rejected_frame.is_none()
            && self.output.is_none()
    }

    /// Borrow the first original owner failure without exposing sensitive payloads.
    pub(crate) fn failure(&self) -> Option<&(dyn Error + Send + Sync + 'static)> {
        self.failure.as_deref()
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
    pub(crate) fn next_deadline(&self) -> Option<Moment> {
        if self.settled() {
            return None;
        }
        let ui = self.watchdog.as_ref().and_then(|watchdog| {
            let snapshot = watchdog.snapshot();
            snapshot.suspicion_at.or(snapshot.next_challenge_at)
        });
        [
            self.cleanup.map(|budget| budget.settle_until),
            ui,
            self.profile_pending.as_ref().map(|pending| pending.original),
            self.main_deadline(),
            self.requests.next_deadline(),
        ]
        .into_iter()
        .flatten()
        .min()
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
