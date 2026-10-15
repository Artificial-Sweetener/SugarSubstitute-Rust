// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hold actual profile identity through browser lifetime and checked family retirement.
mod attempt;
mod custody;
mod folder;
mod launch;
mod migration;
mod proof;
pub(crate) mod record;
mod storage;
mod submission;
#[cfg(test)]
mod tests;

use crate::registry::{RegistryIdentity, lock};
use crate::{ClaimAdmissionError, ClaimError, LaunchTicket, WindowsProcessRegistry};
pub use launch::ProfileDesktopLaunch;
pub use proof::{BrowserFamilyProof, EnvironmentProfileProof, ProfileFolderFacts};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    },
};
use sugarsubstitute_contracts::{
    execution::{CompletionBatch, ExecutionId, JobId, JobSpec, OwnedJob, Scope, TerminalKind},
    lifecycle::RunKey,
    startup::MainIdentity,
};
use sugarsubstitute_execution::ExecutionClient;

/// Peak reservation for one admitted profile operation, including persistent storage.
pub const PROFILE_OPERATION_BYTES: u64 = 2 * 1024 * 1024;
/// Count profile-owned inline state, compact-carrier allocation and original bounded buffers.
pub(crate) const fn carrier_metadata_bytes() -> u64 {
    (size_of::<storage::NativeState>()
        + size_of::<crate::registry::profile::prepared::PreparedDesktop>()
        + size_of::<crate::registry::profile::prepared::PreparedDesktopCarrier>()
        + 2 * 65532
        + 2 * 8192
        + 2 * size_of::<usize>()) as u64
}
const INITIAL: u8 = 0;
const HELD: u8 = 1;
const RELEASED: u8 = 2;
const UNCERTAIN: u8 = 3;

/// Observable ownership facts, independent of a finite worker's terminal classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileSnapshot {
    /// The stable exclusion and held record remain owned.
    pub held: bool,
    /// Native browser ownership could not be established; automatic release is forbidden.
    pub uncertain: bool,
    /// All actual profile handles have completed checked release.
    pub released: bool,
    /// A finite operation remains owned and unconsumed.
    pub pending: bool,
}
impl ProfileSnapshot {
    /// Permit retirement only after checked release and consumption of the original operation.
    /// A worker can publish `released` before its terminal result reaches this owner.
    pub const fn retirement_ready(self) -> bool {
        self.released && !self.pending
    }
}
/// Transfer a finite operation's actual terminal outcome without granting native authority.
#[derive(Debug)]
pub struct ProfileCompletion {
    /// Exact owned job whose result was consumed.
    pub job: JobId,
    /// Actual execution classification.
    pub kind: TerminalKind,
    /// Original native or worker failure, when present.
    pub failure: Option<ClaimError>,
}
struct Shared {
    native: Mutex<Option<Box<storage::NativeState>>>,
    active: Arc<AtomicU8>,
    observed_held: AtomicBool,
    closing: AtomicBool,
}
impl Drop for Shared {
    fn drop(&mut self) {
        let native = match self.native.get_mut() {
            Ok(value) => value,
            Err(poison) => poison.into_inner(),
        };
        if let Some(state) = native.take() {
            if state.retired {
                drop(state);
            } else {
                std::mem::forget(state);
            }
        }
    }
}
#[derive(Clone, Copy)]
enum LaunchState {
    Unattempted,
    Accepted(LaunchTicket),
    Rejected,
}
/// Retain exclusion, finite operations and exact desktop ownership for one run.
/// Construction is lexical. Native effects occur only in admitted finite worker jobs.
/// Dropping unresolved state retains preallocated ownership without native calls.
#[must_use = "Retain profile ownership until its exact family and profile handles retire"]
pub struct WindowsProfileLease {
    run: RunKey,
    path: PathBuf,
    registry: RegistryIdentity,
    shared: Arc<Shared>,
    prepared: Arc<AtomicU8>,
    pending: Option<OwnedJob<(), ClaimError>>,
    execution: Option<ExecutionId>,
    launch: LaunchState,
    acquire_attempted: bool,
    attempts: attempt::Attempts,
    pending_attempt: Option<MainIdentity>,
    pending_browser: bool,
    release_deadline: Option<sugarsubstitute_contracts::execution::Moment>,
    folder: Option<ProfileFolderFacts>,
    environment: Option<EnvironmentProfileProof>,
    browser: Option<BrowserFamilyProof>,
    port: Arc<dyn storage::ProfilePort>,
}
impl WindowsProfileLease {
    /// Create a lexical owner bound to the actual registry lifetime.
    /// # Errors
    /// Rejects nonlocal/malformed paths or a foreign coordinator.
    pub fn new(
        run: RunKey,
        path: PathBuf,
        registry: &WindowsProcessRegistry,
    ) -> Result<Self, ClaimAdmissionError> {
        crate::protected_storage::paths::wide(&path).map_err(ClaimAdmissionError::Native)?;
        let identity = registry.claim_identity(run.owner).map_err(ClaimAdmissionError::Registry)?;
        Ok(Self {
            run,
            path,
            registry: identity,
            shared: Arc::new(Shared {
                native: Mutex::new(Some(Box::new(storage::NativeState::new()))),
                active: Arc::new(AtomicU8::new(INITIAL)),
                observed_held: AtomicBool::new(false),
                closing: AtomicBool::new(false),
            }),
            prepared: Arc::new(AtomicU8::new(0)),
            pending: None,
            execution: None,
            launch: LaunchState::Unattempted,
            acquire_attempted: false,
            attempts: attempt::Attempts::new(),
            pending_attempt: None,
            pending_browser: false,
            release_deadline: None,
            folder: None,
            environment: None,
            browser: None,
            port: Arc::new(storage::WindowsProfilePort),
        })
    }
    /// Observe actual retained native facts; cancellation alone does not change ownership.
    pub fn snapshot(&self) -> ProfileSnapshot {
        let active = self.shared.active.load(Ordering::Acquire);
        ProfileSnapshot {
            held: self.shared.observed_held.load(Ordering::Acquire),
            uncertain: active == UNCERTAIN,
            released: active == RELEASED,
            pending: self.pending.is_some(),
        }
    }
    /// Transfer the current attempt's freshly prepared canonical folder without UI allocation.
    pub fn take_folder_facts(&mut self) -> Option<ProfileFolderFacts> {
        self.folder.take()
    }
    /// Transfer this attempt's prepared environment proof after exact worker completion.
    pub fn take_environment_proof(&mut self) -> Option<EnvironmentProfileProof> {
        self.environment.take()
    }
    /// Transfer this attempt's prepared browser proof after exact worker completion.
    pub fn take_browser_proof(&mut self) -> Option<BrowserFamilyProof> {
        self.browser.take()
    }
    /// Consume exact owned completion metadata; unrelated notifications are ignored.
    /// # Errors
    /// Rejects oversized notification batches; preserves an owned unconsumed job.
    pub fn consume(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<Option<ProfileCompletion>, ClaimError> {
        if batch.notifications.len() > 32 {
            return Err(ClaimError::invalid("profile_completion_bound"));
        }
        self.advance_custody_release()?;
        let Some(job) = self.pending.as_mut() else {
            return Ok(None);
        };
        if !batch
            .notifications
            .iter()
            .any(|notice| notice.scope_id == job.scope_id() && notice.job_id == job.spec().id)
        {
            return Ok(None);
        }
        let Some(completion) = job.try_take_completion() else {
            return Ok(None);
        };
        let id = job.spec().id;
        self.pending = None;
        let identity = self.pending_attempt.take();
        let browser = std::mem::replace(&mut self.pending_browser, false);
        let kind = completion.outcome.kind();
        let shared = self.shared.clone();
        let mut native = lock(&shared.native);
        let state =
            native.as_mut().ok_or_else(|| ClaimError::invalid("profile_completion_storage"))?;
        self.shared.observed_held.store(state.held, Ordering::Release);
        let failure = state.failure.take().or_else(|| match completion.outcome {
            sugarsubstitute_contracts::execution::JobOutcome::Failure(cause) => Some(cause),
            sugarsubstitute_contracts::execution::JobOutcome::Panicked(cause) => {
                Some(ClaimError::new(
                    "profile_worker_panic",
                    std::io::Error::other(cause.as_str().to_owned()),
                ))
            }
            _ => None,
        });
        // Proofs never survive cancellation or late completion as successful authorization.
        if kind == TerminalKind::Success
            && failure.is_none()
            && self.shared.active.load(Ordering::Acquire) == HELD
            && !self.shared.closing.load(Ordering::Acquire)
        {
            if let Some(prepared) = state.prepared_folder.take()
                && prepared.binding.live(prepared.binding.identity)
            {
                self.folder = Some(ProfileFolderFacts::new(self, prepared));
            }
            if let Some((binding, job)) = state.environment.take()
                && binding.live(binding.identity)
            {
                self.environment = Some(EnvironmentProfileProof::new(self, binding, job));
            }
            if let Some((binding, job, native_identity, family)) = state.browser.take()
                && binding.live(binding.identity)
            {
                self.browser =
                    Some(BrowserFamilyProof::new(self, binding, job, native_identity, family));
            }
        } else if kind != TerminalKind::Success
            || failure.is_some()
            || self.shared.closing.load(Ordering::Acquire)
        {
            state.prepared_folder = None;
            state.environment = None;
            state.browser = None;
            if let Some(identity) = identity {
                self.attempts
                    .retire(identity)
                    .map_err(|e| ClaimError::new("profile_attempt_retirement", e))?;
            } else {
                self.shared.closing.store(true, Ordering::Release);
            }
            if browser && self.shared.active.load(Ordering::Acquire) != RELEASED {
                self.shared.active.store(UNCERTAIN, Ordering::Release);
            }
        }
        Ok(Some(ProfileCompletion { job: id, kind, failure }))
    }
    fn registry_matches(
        &self,
        registry: &WindowsProcessRegistry,
    ) -> Result<(), ClaimAdmissionError> {
        let identity =
            registry.claim_identity(self.run.owner).map_err(ClaimAdmissionError::Registry)?;
        if self.registry.same_as(&identity) { Ok(()) } else { Err(ClaimAdmissionError::Identity) }
    }
}
