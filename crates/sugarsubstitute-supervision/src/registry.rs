// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own admitted Windows process families independently of job cancellation/results.
//! The native caller authenticates coordinator/bindings and resolves managed trust.
//! IDs here are matching facts. Only exact owned families can be stopped; attached
//! processes have no registry entry. Normal teardown settles and retires every
//! record before executor shutdown. Drop is owned containment, never clean proof.
use crate::{
    FamilyBinding, LaunchTicket, RegistryAdmissionError, RegistryError, RegistryObservation,
    RegistrySnapshot, RootObservationOutcome, WindowsLaunchInput,
    native_process::{self, Family, Identity, NativePort, WatchInput},
};
use std::sync::{
    Arc, Mutex, MutexGuard,
    atomic::{AtomicU8, Ordering},
};
pub(crate) use submission::validate_blocking;
use sugarsubstitute_contracts::{
    execution::{CompletionBatch, ExecutionPort, JobContext, JobId, JobSpec, OwnedJob, Scope},
    lifecycle::{CoordinatorKey, ProcessId, ProcessKey, ShutdownBudget, ShutdownId},
};
use sugarsubstitute_execution::{
    ExecutionClient,
    windows::{NativeObservationError, ProcessExit, ProcessWatchPermit},
};

const OPEN: u8 = 0;
const RESUME_COMMITTED: u8 = 1;
const CLOSED: u8 = 2;
pub(crate) fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    match value.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}
type WatchJob = OwnedJob<ProcessExit, NativeObservationError>;
struct Shared {
    admission: AtomicU8,
    native: Mutex<NativeState>,
    port: Arc<dyn NativePort>,
    permit: Mutex<Option<ProcessWatchPermit>>,
}
struct NativeState {
    family: Option<Box<Family>>,
    watch: Option<WatchJob>,
    creation: Option<CreationFacts>,
    cleanup: Option<CleanupFacts>,
    failure: Option<RegistryError>,
    forced: bool,
    port: Arc<dyn NativePort>,
}
impl Drop for NativeState {
    fn drop(&mut self) {
        drop(self.watch.take());
        if let Some(mut family) = self.family.take()
            && let Err(cause) = self.port.close(&mut family)
        {
            tracing::error!(operation = "registry_owner_loss_close", cause = %cause, "Failed native retirement retains bounded storage and lease without retry authority");
            // Only a specific failed kernel close retains this fixed native state
            // and its real charge. This does not report family settlement.
            std::mem::forget(family);
        }
    }
}
#[derive(Clone, Copy)]
struct CreationFacts {
    root: Identity,
    resumed: bool,
}
#[derive(Clone, Copy)]
struct CleanupFacts {
    retained: usize,
    root: Option<Identity>,
    next: Option<(usize, Identity)>,
    watched: Option<Identity>,
    closed: bool,
}
struct Record {
    ticket: LaunchTicket,
    shared: Arc<Shared>,
    native_scope: Scope,
    rejected_launch: bool,
    creation: Option<OwnedJob<(), RegistryError>>,
    root_spec: (sugarsubstitute_contracts::execution::ScopeId, JobId),
    root_watch: Option<WatchJob>,
    root_notice: bool,
    root: Option<Identity>,
    root_exit: Option<ProcessExit>,
    last_member_exit: Option<ProcessExit>,
    forced: bool,
    root_retirement: Option<(JobId, RootObservationOutcome)>,
    member_watch: Option<WatchJob>,
    member_spec: Option<(sugarsubstitute_contracts::execution::ScopeId, JobId)>,
    member_notice: bool,
    watched: Option<Identity>,
    pending_exit: Option<Identity>,
    cleanup: Option<OwnedJob<(), RegistryError>>,
    stop: Option<(ShutdownId, ShutdownBudget)>,
    retained: usize,
    next: Option<(usize, Identity)>,
    closed_native: bool,
    started: bool,
    settled: bool,
    error: Option<RegistryError>,
    unread: [bool; 5],
}
impl Record {
    fn fail(&mut self, error: RegistryError) {
        match &mut self.error {
            Some(first) => first.retain(error),
            None => self.error = Some(error),
        }
        self.unread[3] = true;
    }
    fn snapshot(&self) -> RegistrySnapshot {
        RegistrySnapshot {
            process: self.ticket.process,
            binding: self.ticket.binding,
            creation_pending: self.creation.is_some(),
            started: self.started,
            launch_closed: self.shared.admission.load(Ordering::Acquire) == CLOSED,
            root_exit: self.root_exit.map(|exit| exit.exit_code),
            last_member_exit: self.last_member_exit,
            forced: self.forced,
            retained_members: self.retained,
            cleanup_pending: self.cleanup.is_some() || self.member_watch.is_some(),
            member_watch_required: self.next.is_some() && self.member_watch.is_none(),
            settled: self.settled,
            failed: self.error.is_some(),
            shutdown: self.stop,
        }
    }
}
/// Own up to64 desktop and64 managed family records, including pending/uncertain cleanup.
/// Methods submit finite work without native UI waits. The injected execution owner
/// alone chooses workers; this adapter exports no thread/runtime/wait-registration API.
#[must_use = "Keep the registry alive until actual family settlement and observation transfer"]
pub struct WindowsProcessRegistry {
    owner: CoordinatorKey,
    identity: RegistryIdentity,
    records: Box<[Option<Record>]>,
    next_process: u64,
    launch_open: bool,
    cursor: usize,
    cleanup_watches: usize,
    port: Arc<dyn NativePort>,
}
impl WindowsProcessRegistry {
    /// Construct an empty native registry for a trusted current coordinator lifetime.
    /// The fixed record table uses checked heap storage, keeping composition off the
    /// native main-thread stack. Allocation failure preserves its original cause.
    /// No native effect, process enumeration or environment capture occurs.
    pub fn new(owner: CoordinatorKey) -> Result<Self, RegistryError> {
        let mut records = Vec::new();
        records
            .try_reserve_exact(128)
            .map_err(|cause| RegistryError::new("registry_records_allocation", cause))?;
        records.resize_with(128, || None);
        Ok(Self {
            owner,
            identity: RegistryIdentity::new(),
            records: records.into_boxed_slice(),
            next_process: 1,
            launch_open: true,
            cursor: 0,
            cleanup_watches: 0,
            port: native_process::production(),
        })
    }
    /// Stop all future family admissions and close pending exact resume commitments.
    /// Already-linearized resume/native work remains owned and requires actual cleanup.
    pub fn close_launch_admission(&mut self) {
        self.launch_open = false;
        for record in self.records.iter().flatten() {
            record.shared.admission.store(CLOSED, Ordering::Release);
        }
    }
    fn index(&self, key: ProcessKey) -> Result<usize, RegistryAdmissionError> {
        if key.owner != self.owner {
            return Err(RegistryAdmissionError::ForeignOwner);
        }
        self.records
            .iter()
            .position(|record| record.as_ref().is_some_and(|r| r.ticket.process == key))
            .ok_or(RegistryAdmissionError::Stale)
    }
    /// Inspect cached facts only; no PID query or cancellation inference is performed.
    pub fn snapshot(&self, key: ProcessKey) -> Result<RegistrySnapshot, RegistryAdmissionError> {
        let index = self.index(key)?;
        self.records[index].as_ref().map(Record::snapshot).ok_or(RegistryAdmissionError::Stale)
    }
    /// Freeze one exact stop transaction and close launch admission before cleanup.
    /// Only the canonical start+10s/+15s budget is accepted; repeats cannot reset it.
    pub fn begin_stop(
        &mut self,
        key: ProcessKey,
        shutdown: ShutdownId,
        budget: ShutdownBudget,
    ) -> Result<(), RegistryAdmissionError> {
        if ShutdownBudget::new(budget.started).ok() != Some(budget) {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        let index = self.index(key)?;
        let record = self.records[index].as_mut().ok_or(RegistryAdmissionError::Stale)?;
        if let Some(previous) = record.stop {
            return if previous == (shutdown, budget) {
                Ok(())
            } else {
                Err(RegistryAdmissionError::InvalidSpecification)
            };
        }
        record.shared.admission.store(CLOSED, Ordering::Release);
        record.stop = Some((shutdown, budget));
        Ok(())
    }
    /// Transfer one cached actual observation; first failure never overwrites its cause.
    pub fn take_observation(
        &mut self,
        key: ProcessKey,
    ) -> Result<Option<RegistryObservation>, RegistryAdmissionError> {
        let index = self.index(key)?;
        let record = self.records[index].as_mut().ok_or(RegistryAdmissionError::Stale)?;
        if record.unread[0] {
            record.unread[0] = false;
            return Ok(Some(RegistryObservation::ContainedStarted { process: key }));
        }
        if record.unread[1] {
            record.unread[1] = false;
            if let Some(exit) = record.root_exit {
                return Ok(Some(RegistryObservation::RootExited {
                    process: key,
                    code: exit.exit_code,
                }));
            }
        }
        if record.unread[4] {
            record.unread[4] = false;
            if let Some((job, outcome)) = record.root_retirement {
                return Ok(Some(RegistryObservation::RootObservationRetired {
                    process: key,
                    job,
                    outcome,
                }));
            }
        }
        if record.unread[3] {
            record.unread[3] = false;
            if let Some(error) = &record.error {
                return Ok(Some(RegistryObservation::Failed {
                    process: key,
                    error: error.clone(),
                }));
            }
        }
        if record.unread[2] {
            record.unread[2] = false;
            return Ok(Some(RegistryObservation::FamilySettled {
                process: key,
                shutdown: record.stop.map(|s| s.0),
            }));
        }
        Ok(None)
    }
    /// Retire only an actually settled record after every cached observation transferred.
    /// Retired ProcessIds never become reusable within this coordinator registry.
    pub fn retire_settled(&mut self, key: ProcessKey) -> Result<(), RegistryAdmissionError> {
        let index = self.index(key)?;
        let record = self.records[index].as_ref().ok_or(RegistryAdmissionError::Stale)?;
        if !record.settled || record.unread.iter().any(|unread| *unread) {
            return Err(RegistryAdmissionError::Busy);
        }
        self.records[index] = None;
        Ok(())
    }
    /// Report whether every family has settled and been explicitly retired.
    pub fn is_empty(&self) -> bool {
        self.records.iter().all(Option::is_none)
    }
}
impl Drop for WindowsProcessRegistry {
    fn drop(&mut self) {
        self.close_launch_admission();
        for record in self.records.iter().flatten() {
            tracing::error!(operation = "registry_owner_loss", process_id = record.ticket.process.id.get(), epoch = ?self.owner.epoch,
                settled = record.settled, "Registry dropped before explicit family retirement; owned containment teardown requested");
            if let Some(job) = &record.creation {
                job.cancel();
            }
            if let Some(job) = &record.cleanup {
                job.cancel();
            }
            if let Some(job) = &record.root_watch {
                job.cancel();
            }
            if let Some(job) = &record.member_watch {
                job.cancel();
            }
        }
    }
}
mod claim_identity;
mod cleanup;
pub(crate) use claim_identity::RegistryIdentity;
mod completion;
mod observation_retirement;
mod submission;
#[cfg(test)]
mod tests;
