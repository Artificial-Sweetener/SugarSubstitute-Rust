// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain durable native state before callbacks and restore it on every unwind.
use super::{control::WitnessControl, entry::Entry, pool::WitnessPool};
use crate::{ClaimError, native_claims::files::Handle};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use sugarsubstitute_contracts::backend_custody::{BackendCustodyFrame, BackendCustodyReceipt};
use sugarsubstitute_execution::backend_custody::{
    BackendCustodyMemory, BackendCustodyProcessWatch,
};

/// Retain the fixed admitted pool and all partial native outputs outside worker unwind.
/// Construction invokes no native effects. Every alias frees actual control/data
/// before the identical original metadata claim can become reusable.
pub struct BackendCustodySource {
    owner: Option<Arc<Mutex<Option<Box<State>>>>>,
    memory: BackendCustodyMemory,
    installation: super::installation::Control,
}
pub(super) struct State {
    pub(super) installation: PathBuf,
    pub(super) trial: super::installation::Control,
    pub(super) directory: PathBuf,
    pub(super) sid: Vec<u8>,
    pub(super) handles: Vec<Handle>,
    pub(super) parent: Option<BackendCustodyProcessWatch>,
    pub(super) containment: Option<Handle>,
    pub(super) transfers: [Option<Handle>; 2],
    pub(super) registrations: [Option<Entry>; 64],
    pub(super) latest: Option<BackendCustodyFrame>,
    pub(super) reply: Option<BackendCustodyReceipt>,
    pub(super) reply_until: Option<sugarsubstitute_contracts::execution::Moment>,
    pub(super) initialized: bool,
    pub(super) established: bool,
    pub(super) stopping: bool,
    pub(super) finish_authorized: bool,
    pub(super) stop_until: Option<sugarsubstitute_contracts::execution::Moment>,
    pub(super) settled: bool,
    pub(super) failed: bool,
    pub(super) first: Option<ClaimError>,
    pub(super) secondary: [Option<ClaimError>; 8],
    pub(super) omissions: u64,
    pub(super) pool: WitnessPool,
    pub(super) control: WitnessControl,
}
impl BackendCustodySource {
    /// Fund every original slot and preflight metadata before source allocation.
    /// # Errors
    /// Preserves genuine original capacity/provider failures with the same guard.
    pub fn new(
        memory: &BackendCustodyMemory,
        installation: PathBuf,
        wake: Arc<dyn sugarsubstitute_contracts::execution::CompletionWake>,
    ) -> Result<Self, BackendCustodyFailure> {
        memory.begin_source().map_err(|cause| BackendCustodyFailure {
            cause: FailureCause::Admission(cause),
            memory: memory.clone(),
        })?;
        let construct = || -> Result<Self, ClaimError> {
            crate::protected_storage::paths::validate_local_shape(&installation)
                .map_err(ClaimError::invalid)?;
            let storage = size_of::<State>() + 2 * size_of::<usize>() + size_of::<Mutex<Option<Box<State>>>>()
                + sugarsubstitute_execution::backend_custody::BackendCustodyWorkerStart::control_storage_bytes()
                + 256 * size_of::<Handle>() + 512 * 1024
                + super::installation::Control::storage_bytes(&installation);
            if storage > memory.bytes() as usize {
                return Err(ClaimError::invalid("custody_source_capacity"));
            }
            let pool = WitnessPool::new(memory)?;
            let control = WitnessControl::new(memory);
            let trial = super::installation::Control::new(
                crate::native_claims::files::Budget::CustodyCore(control.clone()),
                installation.clone(),
                wake,
            );
            let mut handles = Vec::new();
            handles
                .try_reserve_exact(256)
                .map_err(|cause| ClaimError::new("custody_source_handles", cause))?;
            Ok(Self {
                owner: Some(Arc::new(Mutex::new(Some(Box::new(State {
                    installation,
                    trial: trial.clone(),
                    directory: PathBuf::new(),
                    sid: Vec::new(),
                    handles,
                    parent: None,
                    containment: None,
                    transfers: [None, None],
                    registrations: std::array::from_fn(|_| None),
                    latest: None,
                    reply: None,
                    reply_until: None,
                    initialized: false,
                    established: false,
                    stopping: false,
                    finish_authorized: false,
                    stop_until: None,
                    settled: false,
                    failed: false,
                    first: None,
                    secondary: std::array::from_fn(|_| None),
                    omissions: 0,
                    pool,
                    control,
                }))))),
                memory: memory.clone(),
                installation: trial,
            })
        };
        construct().map_err(|cause| BackendCustodyFailure {
            cause: FailureCause::Native(cause),
            memory: memory.clone(),
        })
    }
    /// Borrow the same original native state outside every source/Core mutex.
    pub(super) fn access(&self) -> Result<Access, ClaimError> {
        let owner =
            self.owner.as_ref().ok_or_else(|| ClaimError::invalid("custody_source_closed"))?;
        let state = super::super::lock(owner)
            .take()
            .ok_or_else(|| ClaimError::invalid("custody_source_callback_pending"))?;
        Ok(Access { state: Some(state), original: self.clone() })
    }
    pub(super) fn memory(&self) -> &BackendCustodyMemory {
        &self.memory
    }
    /// Transfer the sole storage-job observer to the helper lifetime before binding its driver.
    /// # Errors
    /// Rejects repeated acquisition; the source retains all native request and witness custody.
    pub fn take_installation_worker(
        &self,
    ) -> Result<super::BackendCustodyInstallation, ClaimError> {
        self.installation.worker()
    }
    /// Clone the retained first actual cause without another cause allocation or effect.
    /// The same original outside metadata guards every escaped observation.
    pub fn failure(&self) -> Option<BackendCustodyFailure> {
        let owner = self.owner.as_ref()?;
        let cause = super::super::lock(owner).as_ref()?.first.clone()?;
        Some(BackendCustodyFailure {
            cause: FailureCause::Native(cause),
            memory: self.memory.clone(),
        })
    }
    /// Read cached recovery capture custody; uncertainty conservatively retains the helper.
    /// This observation does not authorize native effects or establish capture success.
    pub fn recovery_capture_pending(&self) -> bool {
        self.owner.as_ref().is_none_or(|owner| {
            super::super::lock(owner).as_ref().is_none_or(|state| {
                state.registrations.iter().flatten().any(super::entry::Entry::capture_pending)
            })
        })
    }
    /// Return the immutable outer bound while recovery capture is still pending.
    /// No actual settlement is inferred from reaching this limit.
    pub fn recovery_retirement_until(
        &self,
    ) -> Option<sugarsubstitute_contracts::execution::Moment> {
        let owner = self.owner.as_ref()?;
        super::super::lock(owner)
            .as_ref()?
            .registrations
            .iter()
            .flatten()
            .filter_map(super::entry::Entry::recovery_settle_until)
            .min()
    }
    /// Return only the actual helper control's first post-capture containment cutoff.
    /// In-flight native callbacks cannot hide this published observation.
    pub fn retirement_until(&self) -> Option<sugarsubstitute_contracts::execution::Moment> {
        self.memory.cleanup_until()
    }
    /// Observe only actual cached unresolved custody; it grants no Released authority.
    pub fn unresolved(&self) -> bool {
        self.owner.as_ref().is_none_or(|owner| {
            super::super::lock(owner).as_ref().is_none_or(|state| !state.settled)
        })
    }
}
impl Clone for BackendCustodySource {
    fn clone(&self) -> Self {
        Self {
            owner: self.owner.clone(),
            memory: self.memory.clone(),
            installation: self.installation.clone(),
        }
    }
}
impl Drop for BackendCustodySource {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.take()
            && let Some(state) = Arc::into_inner(owner)
        {
            let held = state.into_inner().unwrap_or_else(std::sync::PoisonError::into_inner);
            if held.as_ref().is_some_and(|state| {
                !state.settled || state.containment.as_ref().is_some_and(Handle::is_armed)
            }) {
                // The independently retained self-containment outlives worker
                // ACK/join until this verified headless process actually exits.
                std::mem::forget((held, self.memory.clone()));
            } else {
                drop(held);
            }
        }
    }
}
/// Take/reinstall stable state; unexpected unwind cannot discard native outputs.
pub(super) struct Access {
    pub(super) state: Option<Box<State>>,
    original: BackendCustodySource,
}
impl Access {
    pub(super) fn state(&mut self) -> Result<&mut State, ClaimError> {
        self.state.as_deref_mut().ok_or_else(|| ClaimError::invalid("custody_source_access_closed"))
    }
}
impl Drop for Access {
    fn drop(&mut self) {
        if let Some(owner) = &self.original.owner {
            *super::super::lock(owner) = self.state.take();
        }
    }
}
/// Preserve the actual failure before its unchanged outside metadata guard.
pub struct BackendCustodyFailure {
    cause: FailureCause,
    memory: BackendCustodyMemory,
}
enum FailureCause {
    Native(ClaimError),
    Admission(sugarsubstitute_contracts::execution::AdmissionError),
}
impl BackendCustodyFailure {
    /// Borrow matching original session facts without granting process authority.
    pub fn key(&self) -> Option<sugarsubstitute_contracts::backend_custody::BackendCustodyKey> {
        self.memory.key()
    }
}
impl std::fmt::Debug for BackendCustodyFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.cause {
            FailureCause::Native(cause) => cause.fmt(f),
            FailureCause::Admission(cause) => cause.fmt(f),
        }
    }
}
impl std::fmt::Display for BackendCustodyFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.cause {
            FailureCause::Native(cause) => cause.fmt(f),
            FailureCause::Admission(cause) => cause.fmt(f),
        }
    }
}
impl std::error::Error for BackendCustodyFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match &self.cause {
            FailureCause::Native(cause) => cause,
            FailureCause::Admission(cause) => cause,
        })
    }
}
