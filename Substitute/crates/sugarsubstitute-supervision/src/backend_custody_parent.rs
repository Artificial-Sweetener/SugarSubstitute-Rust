// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Authenticate one actual helper and retain the original bounded custody transactions.
use crate::native_claims::lock;
use std::{
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};
use sugarsubstitute_contracts::{backend_custody::*, execution::Moment};
use sugarsubstitute_execution::{backend_custody::*, windows::NativeObservationError};
mod error;
mod finish;
mod heartbeat;
#[cfg(test)]
pub(crate) mod observation_fixture;
mod receipt;
mod retirement;
mod state;
pub use error::{BackendCustodyParentFailure, BackendCustodyParentInputError};
use receipt::StoredReceipt;
pub use retirement::BackendCustodyFinishStatus;
use state::{State, reject};
/// Keep the complete actual controller allocation before its original pipe metadata.
pub struct BackendCustodyParent {
    state: Option<Arc<Mutex<Option<Box<State>>>>>,
    storage: Arc<BackendCustodyParentStorage>,
}
/// Actual correlated response consumed by its original controller.
/// Every escaped alias retains that same controller allocation and metadata.
#[derive(Clone)]
pub struct BackendCustodyAcknowledgement {
    facts: ReceiptFacts,
    parent: BackendCustodyParent,
}
#[derive(Clone, Copy)]
struct ReceiptFacts {
    key: BackendCustodyKey,
    nonce: u64,
    receipt: StoredReceipt,
}
impl BackendCustodyAcknowledgement {
    /// Match the one actual originating parent allocation without forming an owner cycle.
    pub(crate) fn matches_storage(&self, storage: &BackendCustodyParentStorage) -> bool {
        self.parent.storage.same_allocation(storage)
    }
    /// Observe the unchanged source key.
    pub const fn key(&self) -> BackendCustodyKey {
        self.facts.key
    }
    /// Observe the exact original command nonce.
    pub const fn nonce(&self) -> u64 {
        self.facts.nonce
    }
    /// Observe the actual correlated helper receipt; native owners bind their cells too.
    pub const fn receipt(&self) -> BackendCustodyReceipt {
        self.facts.receipt.expand(self.facts.key.owner())
    }
}
impl BackendCustodyParent {
    /// Revalidate the actual original authenticated helper source before application effects.
    /// # Errors
    /// Preserves original provider, cancellation, stopped-transport and authentication causes.
    pub fn validate_current_admission(&self) -> Result<(), BackendCustodyParentFailure> {
        self.check().map(|_| ())
    }
    /// Observe actual native preparation before transferring bootstrap pins.
    pub fn prepared(&self) -> bool {
        self.state
            .as_ref()
            .is_some_and(|state| lock(state).as_ref().is_some_and(|state| state.prepared))
    }
    /// Transfer the genuine once-only bootstrap/enrollment pair for this helper launch.
    /// # Errors
    /// Preserves actual transport ownership/cancellation errors without recreating pins.
    pub fn take_bootstrap(
        &self,
    ) -> Result<
        Option<(
            sugarsubstitute_execution::windows_backend_custody::BackendCustodyBootstrap,
            sugarsubstitute_execution::windows_backend_custody::BackendCustodyEnrollment,
        )>,
        BackendCustodyParentFailure,
    > {
        self.access(|state| {
            state
                .session
                .take_bootstrap()
                .map_err(|cause| NativeObservationError::new("custody_parent_bootstrap", cause))
        })
    }
    /// Borrow the genuine fixed metadata source for the same finite creation admission.
    pub(crate) fn storage(&self) -> &BackendCustodyParentStorage {
        &self.storage
    }
    /// Compare actual execution provenance without projecting a scheduler or native handle.
    pub(crate) fn matches_execution(
        &self,
        execution: &sugarsubstitute_execution::ExecutionClient,
    ) -> bool {
        self.storage.matches_execution(execution)
    }
    /// Preflight and adopt the one original server, preserving the complete option on rejection.
    /// It uses this session's existing64KiB metadata; no new reservation/claim is issued.
    /// # Errors
    /// Rejects missing/repeated/foreign or oversized original allocation before effects.
    pub fn new(
        session: &mut Option<BackendCustodyPipeSession>,
    ) -> Result<Self, BackendCustodyParentInputError> {
        let original = session.as_ref().ok_or(BackendCustodyParentInputError::Missing)?;
        let required = size_of::<State>()
            + size_of::<Mutex<Option<Box<State>>>>()
            + 2 * size_of::<usize>()
            + BackendCustodyParentStorage::producer_storage_bytes()
            + 66 * crate::native_process::custody_transfer::CustodyDestination::storage_bytes()
            + crate::registry::custody_establishment_storage_bytes();
        let required = required
            + size_of::<BackendCustodyParentStorage>()
            + 2 * size_of::<usize>()
            + size_of::<Self>();
        if required > 65_536 {
            return Err(BackendCustodyParentInputError::Layout);
        }
        let storage = original
            .control()
            .take_parent_storage()
            .map_err(BackendCustodyParentInputError::Admission)?;
        let original = session.take().ok_or(BackendCustodyParentInputError::Missing)?;
        Ok(Self {
            state: Some(Arc::new(Mutex::new(Some(Box::new(State {
                session: original,
                prepared: false,
                peer: None,
                read: None,
                write: None,
                latest: 0,
                nonce: 1,
                pending: None,
                receipt: None,
                failure: None,
                closing: false,
                established: false,
                registrations: [None; 64],
                finish: None,
                heartbeat: heartbeat::Heartbeat::default(),
            }))))),
            storage: Arc::new(storage),
        })
    }
    /// Borrow matching original purpose/session facts.
    pub fn key(&self) -> BackendCustodyKey {
        self.storage.key()
    }
    /// Drain at most32 actual facts and advance only the original transaction.
    /// # Errors
    /// Preserves native/protocol errors under the unchanged outside metadata guard.
    pub fn consume(&self, max: NonZeroUsize) -> Result<(), BackendCustodyParentFailure> {
        if let Some(cause) = self.cached_failure() {
            return Err(self.failure(cause));
        }
        self.try_access(|state| state.consume(max, self.key()))?;
        Ok(())
    }
    /// Observe cached authenticated readiness, without granting effect authority.
    pub fn authenticated(&self) -> bool {
        self.state.as_ref().is_some_and(|state| {
            lock(state).as_ref().is_some_and(|state| {
                state.peer.is_some() && !state.closing && state.failure.is_none()
            })
        })
    }
    /// Transfer one actual correlated receipt, retaining its original source association.
    pub fn take_acknowledgement(&self) -> Option<BackendCustodyAcknowledgement> {
        self.state
            .as_ref()
            .and_then(|state| lock(state).as_mut().and_then(|state| state.receipt.take()))
            .map(|facts| BackendCustodyAcknowledgement { facts, parent: self.clone() })
    }
    /// Observe only this original helper's actual Establish receipt.
    pub(crate) fn established(&self) -> bool {
        self.state.as_ref().is_some_and(|state| {
            lock(state)
                .as_ref()
                .is_some_and(|state| state.established && !state.closing && state.failure.is_none())
        })
    }
    /// Take only the receipt belonging to this finite owner's original nonce.
    pub(crate) fn take_acknowledgement_for(
        &self,
        nonce: u64,
    ) -> Option<BackendCustodyAcknowledgement> {
        self.state
            .as_ref()
            .and_then(|state| {
                lock(state).as_mut().and_then(|state| {
                    if state.receipt.is_some_and(|facts| facts.nonce == nonce) {
                        state.receipt.take()
                    } else {
                        None
                    }
                })
            })
            .map(|facts| BackendCustodyAcknowledgement { facts, parent: self.clone() })
    }
    /// Admit one controller transaction, explicitly retaining ordinary capacity contention.
    pub(crate) fn try_submit(
        &self,
        command: BackendCustodyCommand,
        until: Moment,
    ) -> Result<Option<u64>, BackendCustodyParentFailure> {
        if let Some(cause) = self.cached_failure() {
            return Err(self.failure(cause));
        }
        self.storage.check().map_err(|cause| self.failure(cause))?;
        Ok(self
            .try_access(|state| {
                if state.closing || state.peer.is_none() {
                    return Err(reject("custody_parent_closed"));
                }
                if state.pending.is_some() || state.receipt.is_some() {
                    return Ok(None);
                }
                let nonce =
                    state.nonce.checked_add(1).ok_or_else(|| reject("custody_parent_nonce"))?;
                let frame = BackendCustodyFrame { key: self.key(), nonce, command };
                frame
                    .encode()
                    .map_err(|cause| NativeObservationError::new("custody_parent_frame", cause))?;
                state.nonce = nonce;
                state.pending = Some((frame, until));
                state.advance()?;
                Ok(Some(nonce))
            })?
            .flatten())
    }
    /// Borrow only the cached absence of another actual controller transaction.
    pub(crate) fn transaction_available(&self) -> bool {
        self.state.as_ref().is_some_and(|state| {
            lock(state).as_ref().is_some_and(|state| {
                state.pending.is_none() && state.receipt.is_none() && !state.closing
            })
        })
    }
    /// Recheck current actual source before a native parent's next finite effect.
    pub(crate) fn check(&self) -> Result<VerifiedBackendCustodyPeer, BackendCustodyParentFailure> {
        self.storage.peer().map_err(|cause| self.failure(cause))
    }
    /// Close new transactions while retaining actual session/response/native source custody.
    pub fn close(&self) -> Result<(), BackendCustodyParentFailure> {
        self.access(|state| {
            state.closing = true;
            state.session.control().close();
            Ok(())
        })
    }
    /// Take original pipe terminal only after the actual callbacks, handles and loans settle.
    /// A concurrent short state loan remains pending; actual observation failures stay errors.
    pub fn take_terminal(
        &self,
    ) -> Result<
        Option<
            sugarsubstitute_contracts::execution::JobCompletion<
                BackendCustodyPipeReport,
                LifecyclePipeFailure,
            >,
        >,
        BackendCustodyParentFailure,
    > {
        Ok(self.try_access(|state| Ok(state.session.take_terminal()))?.flatten())
    }
    /// Take/reinstall the identical stable owner outside provider/native calls.
    fn access<T>(
        &self,
        run: impl FnOnce(&mut State) -> Result<T, NativeObservationError>,
    ) -> Result<T, BackendCustodyParentFailure> {
        let owner =
            self.state.as_ref().ok_or_else(|| self.failure(reject("custody_parent_closed")))?;
        let state = lock(owner)
            .take()
            .ok_or_else(|| self.failure(reject("custody_parent_callback_pending")))?;
        let mut loan = Loan { state: Some(state), parent: self.clone() };
        let original = loan
            .state
            .as_deref_mut()
            .ok_or_else(|| self.failure(reject("custody_parent_state")))?;
        let result = run(original);
        if let Err(cause) = &result {
            original.retain_failure(self.key(), cause);
            original.closing = true;
            original.session.control().close();
        }
        result.map_err(|cause| self.failure(cause))
    }
    /// Concurrent pumps defer while the identical state is on its existing short loan.
    fn try_access<T>(
        &self,
        run: impl FnOnce(&mut State) -> Result<T, NativeObservationError>,
    ) -> Result<Option<T>, BackendCustodyParentFailure> {
        let owner =
            self.state.as_ref().ok_or_else(|| self.failure(reject("custody_parent_closed")))?;
        let Some(state) = lock(owner).take() else {
            return Ok(None);
        };
        let mut loan = Loan { state: Some(state), parent: self.clone() };
        let original = loan
            .state
            .as_deref_mut()
            .ok_or_else(|| self.failure(reject("custody_parent_state")))?;
        let result = run(original);
        if let Err(cause) = &result {
            original.retain_failure(self.key(), cause);
            original.closing = true;
            original.session.control().close();
        }
        result.map(Some).map_err(|cause| self.failure(cause))
    }
    /// Borrow the bounded first actual payload without allocating another cause.
    fn cached_failure(&self) -> Option<NativeObservationError> {
        self.state
            .as_ref()
            .and_then(|state| lock(state).as_ref().and_then(|state| state.failure.clone()))
    }
    /// Pair the same actual source cause and its original metadata through every alias.
    fn failure(&self, cause: NativeObservationError) -> BackendCustodyParentFailure {
        BackendCustodyParentFailure { cause, storage: self.storage.clone() }
    }
}
impl Clone for BackendCustodyParent {
    fn clone(&self) -> Self {
        Self { state: self.state.clone(), storage: self.storage.clone() }
    }
}
impl Drop for BackendCustodyParent {
    fn drop(&mut self) {
        if let Some(owner) = self.state.take() {
            drop(Arc::into_inner(owner));
        }
    }
}
struct Loan {
    state: Option<Box<State>>,
    parent: BackendCustodyParent,
}
impl Drop for Loan {
    fn drop(&mut self) {
        if let Some(owner) = &self.parent.state {
            *lock(owner) = self.state.take();
        }
    }
}
