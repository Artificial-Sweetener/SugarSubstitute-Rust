// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve one actual registered startup specification beyond its original Entry loan.
pub(crate) mod admission;
pub(crate) mod recovery;
mod witness;
use super::{EntryTransportError, PreparedStartupSnapshot, StartupAdmissionBinding, protocol};
use crate::{
    admission::{Core, State},
    scope::lock,
};
pub use recovery::{RecoveryGrantCustody, RecoveryGrantRejection, RegisteredRecoveryGrant};
use std::sync::{Arc, Mutex};
use sugarsubstitute_contracts::{
    coordinator_entry::{EntryLaunchRequestView, EntrySetupBudget},
    execution::{JobId, MemoryClaim, Moment, ScopeId},
    lifecycle::RunKey,
};
pub use witness::RegisteredRestartAdmission;

/// Original and effective immutable bodies plus control from the same 4 MiB projection.
pub const REGISTERED_STARTUP_BYTES: u64 = 2 * 65_536 + 4096;
pub(crate) const PROJECTION_BYTES: u64 = 4 * 1024 * 1024;

pub(super) struct StateOwned {
    pub(super) core: Arc<Core>,
    pub(super) entry: StartupAdmissionBinding,
    pub(super) original_scope: ScopeId,
    pub(super) body: Box<[u8]>,
    pub(super) child_body: Box<[u8]>,
    pub(super) progress: Mutex<Progress>,
    _memory: MemoryClaim,
}
pub(super) struct Progress {
    pub(super) current: RunKey,
    pub(super) issued: Option<(JobId, ScopeId, EntrySetupBudget)>,
    pub(super) recovery_issued:
        Option<sugarsubstitute_contracts::recovery_supervisor::RecoveryGrantKey>,
}

/// Closed historical specification issued only by the actual authenticated Entry producer.
/// Clones retain identical source/claim; the original connection loan is not retained.
/// This type grants no process, election, frontend trust or inference execution privilege.
pub struct RegisteredStartupSource(pub(super) Option<Arc<StateOwned>>);
impl Clone for RegisteredStartupSource {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl Drop for RegisteredStartupSource {
    fn drop(&mut self) {
        if let Some(inner) = self.0.take().and_then(Arc::into_inner) {
            drop(inner);
        }
    }
}
impl RegisteredStartupSource {
    pub(super) fn state(&self) -> Result<&StateOwned, EntryTransportError> {
        self.0.as_deref().ok_or(EntryTransportError::Closed)
    }
    /// Borrow the unchanged startup specification, independently of its original Entry loan.
    /// Shape is inert; each new native capsule requires actual coordinator admission.
    /// # Errors
    /// Rejects missing custody or invalid original specification shape.
    pub fn view(&self) -> Result<EntryLaunchRequestView<'_>, EntryTransportError> {
        protocol::launch_payload::decode(&self.state()?.body)
    }
    /// Return the charged original/effective source envelope; aliases do not enlarge it.
    pub const fn retained_bytes(&self) -> u64 {
        REGISTERED_STARTUP_BYTES
    }
    /// Compare the original physical source allocation without granting admission.
    pub fn same_source(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

#[derive(Clone)]
pub(crate) struct RegisteredSnapshot {
    pub(super) source: RegisteredStartupSource,
    pub(super) previous: RunKey,
    pub(super) run: RunKey,
    pub(super) scope: ScopeId,
    pub(super) job: JobId,
    pub(super) original: EntrySetupBudget,
    pub(super) cancel: Arc<crate::scope::Cancellation>,
    // Original job storage retires before the outside projection reservation.
    _issuer: Arc<dyn crate::task::ErasedJob>,
    _memory: sugarsubstitute_contracts::execution::MemoryLease,
}
impl RegisteredSnapshot {
    pub(crate) fn body(&self) -> &[u8] {
        self.source.0.as_ref().map_or(&[], |source| &source.child_body)
    }
    pub(crate) fn validate_current_locked(
        &self,
        core: &Arc<Core>,
        state: &State,
        run: RunKey,
        now: Moment,
    ) -> Result<Moment, EntryTransportError> {
        let source = self.source.state()?;
        if !Arc::ptr_eq(&source.core, core)
            || run != self.run
            || now < self.original.started()
            || sugarsubstitute_contracts::execution::CancelControl::is_requested(
                self.cancel.as_ref(),
            )
        {
            return Err(EntryTransportError::Authentication);
        }
        let current = lock(&source.progress);
        if current.current != run || current.issued != Some((self.job, self.scope, self.original)) {
            return Err(EntryTransportError::Authentication);
        }
        drop(current);
        let owner_until = source.entry.validate_registered_owner_locked(core, state, run, now)?;
        let original_scope = admission::scope_current(state, source.original_scope)?;
        let scope = admission::scope_current(state, self.scope)?;
        let until = [owner_until, original_scope, scope]
            .into_iter()
            .flatten()
            .fold(self.original.until(), Moment::min);
        if now >= until {
            return Err(EntryTransportError::InvalidDeadline);
        }
        Ok(until)
    }
}

impl PreparedStartupSnapshot {
    pub(super) fn registered(registered: RegisteredSnapshot) -> Result<Self, EntryTransportError> {
        let length = registered.source.state()?.child_body.len();
        Ok(Self {
            launch: None,
            registered: Some(registered),
            delegated: None,
            initial_effective: None,
            body: 0..length,
        })
    }
}
