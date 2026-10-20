// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reuse the original process wait owner under actual custody metadata/slot credit.
use super::*;
use crate::native::{NativeCorrelation, NativeObservationError, ProcessExit, ProcessLease};
use std::os::windows::io::OwnedHandle;

/// Own one parent/member observation on the existing native callback/wait adapter.
/// The durable original process cell precedes registration; Drop retains uncertain
/// callback/handle storage and its complete original credit without native effects.
pub struct BackendCustodyProcessWatch {
    lease: Option<crate::windows::process::Lease>,
    wake: Option<BackendCustodyWake>,
    signal: Option<std::sync::Arc<crate::native::Signal>>,
    expected: Option<(u32, u64)>,
    slot: Option<BackendCustodySlotTurn>,
    metadata: Option<BackendCustodyMemory>,
    watchdog: Option<super::parent_watchdog::ParentWatchdog>,
    termination_issued: bool,
}
impl BackendCustodyProcessWatch {
    /// Include the complete actual lease/wait/control peak before native allocation.
    pub fn storage_bytes() -> usize {
        size_of::<Self>() + crate::windows::process::Lease::storage_bytes()
    }
    /// Adopt the parent duplicate before effects; its expected identity comes only
    /// from the actual authenticated original child-session allocation.
    pub fn parent(input: OwnedHandle, metadata: &BackendCustodyMemory) -> Self {
        Self {
            lease: Some(crate::windows::process::Lease::new(input)),
            wake: None,
            signal: None,
            expected: metadata.parent_identity(),
            slot: None,
            metadata: Some(metadata.clone()),
            watchdog: Some(super::parent_watchdog::ParentWatchdog::default()),
            termination_issued: false,
        }
    }
    /// Adopt one actual membership-verified native source duplicate before effects.
    /// Caller is the custody kernel adapter; expected facts are compared with the
    /// retained process, and provide observation rather than termination authority.
    pub fn member(input: OwnedHandle, expected: (u32, u64), slot: &BackendCustodySlotTurn) -> Self {
        Self {
            lease: Some(crate::windows::process::Lease::new(input)),
            wake: None,
            signal: None,
            expected: Some(expected),
            slot: Some(slot.clone()),
            metadata: Some(slot.metadata().clone()),
            watchdog: None,
            termination_issued: false,
        }
    }
    /// Start one same-worker wait after original ledger/scope validation.
    /// # Errors
    /// Retains partial process/wait outputs, wrong identity and actual provider causes.
    pub fn activate(&mut self, wake: BackendCustodyWake) -> Result<(), NativeObservationError> {
        self.activate_using(wake, None)
    }
    /// Observe an already retained verified family member during original cleanup.
    /// This permits no process creation/resume or renewed registration cutoff.
    /// # Errors
    /// Requires the same turn's close-only proof and preserves partial wait outputs.
    pub fn activate_cleanup(
        &mut self,
        wake: BackendCustodyWake,
        proof: &BackendCustodyCleanup,
    ) -> Result<(), NativeObservationError> {
        self.check_proof(proof)?;
        self.activate_using(wake, Some(proof))
    }
    /// Use one implementation for initial observation and existing-family cleanup.
    fn activate_using(
        &mut self,
        wake: BackendCustodyWake,
        proof: Option<&BackendCustodyCleanup>,
    ) -> Result<(), NativeObservationError> {
        let original = self.metadata.as_ref().ok_or_else(|| failure("custody_watch_credit"))?;
        if !original.matches_wake(&wake) || self.expected.is_none() || self.wake.is_some() {
            return Err(failure("custody_watch_original"));
        }
        let key = original.key().ok_or_else(|| failure("custody_watch_key"))?;
        let signal = original.watch_signal().ok_or_else(|| failure("custody_watch_execution"))?;
        self.signal = Some(signal.clone());
        self.wake = Some(wake);
        let lease = self.lease.as_mut().ok_or_else(|| failure("custody_watch_owner"))?;
        let correlation = NativeCorrelation::new(key.session().get())
            .map_err(|cause| NativeObservationError::new("custody_watch_correlation", cause))?;
        lease.start_checked(correlation, signal, self.expected, || {
            match proof {
                Some(proof) => proof.check(),
                None => original.check_registration().map(|_| ()),
            }
            .map_err(|cause| NativeObservationError::new("custody_watch_control", cause))
        })
    }
    /// Observe only the actual registered callback's original process signal.
    pub fn signaled(&self) -> bool {
        self.signal.as_ref().is_some_and(|signal| signal.completed())
    }
    /// Record a fresh authenticated coordinator command after native identity validation.
    /// # Errors
    /// Rejects member watches and preserves active-clock failures.
    pub fn observe_parent_progress(&mut self) -> Result<(), NativeObservationError> {
        if self.identity() != self.expected || self.expected.is_none() || self.wake.is_none() {
            return Err(failure("parent_watchdog_identity"));
        }
        let now = crate::windows::process::Lease::active_time()?;
        self.watchdog.as_mut().ok_or_else(|| failure("parent_watchdog_member"))?.observe(now);
        Ok(())
    }
    /// Enforce the independent active-time lease using the original authenticated handle.
    /// The returned interval schedules observation; termination never proves actual exit.
    /// # Errors
    /// Preserves native clock/termination causes and rejects member watches.
    pub fn poll_parent_progress(
        &mut self,
    ) -> Result<Option<std::time::Duration>, NativeObservationError> {
        if self.signaled() || self.termination_issued {
            return Ok(None);
        }
        let now = crate::windows::process::Lease::active_time()?;
        let remaining =
            self.watchdog.as_mut().ok_or_else(|| failure("parent_watchdog_member"))?.remaining(now);
        if remaining == Some(std::time::Duration::ZERO) {
            self.lease
                .as_ref()
                .ok_or_else(|| failure("parent_watchdog_owner"))?
                .terminate_unresponsive_parent()?;
            self.termination_issued = true;
            tracing::error!(
                operation = "coordinator_watchdog_expired",
                "Independent custodian terminated its unresponsive coordinator; actual exit and owned cleanup remain pending"
            );
            return Ok(None);
        }
        Ok(remaining)
    }
    /// Borrow cached identity after actual kernel validation, without another query.
    pub fn identity(&self) -> Option<(u32, u64)> {
        self.lease.as_ref().map(|lease| lease.identity())
    }
    /// Read an actual signalled process status under original close-only control.
    /// # Errors
    /// Preserves actual query/provider cause; no status value manufactures exit.
    pub fn read_exit(
        &mut self,
        proof: &BackendCustodyCleanup,
    ) -> Result<ProcessExit, NativeObservationError> {
        self.check_proof(proof)?;
        if !self.signaled() {
            return Err(failure("custody_watch_not_signaled"));
        }
        proof
            .check()
            .map_err(|cause| NativeObservationError::new("custody_watch_cleanup", cause))?;
        self.lease.as_mut().ok_or_else(|| failure("custody_watch_owner"))?.read_exit()
    }
    /// Gate synchronous unregister and each physical close with the same actual proof.
    /// # Errors
    /// Retains original failed/pending outputs; accepted late returns are adopted
    /// before checking whether another fresh effect is permitted.
    pub fn retire(&mut self, proof: &BackendCustodyCleanup) -> Result<(), NativeObservationError> {
        self.check_proof(proof)?;
        if let Some(lease) = &mut self.lease {
            lease.retire_checked(|| {
                proof
                    .check()
                    .map_err(|cause| NativeObservationError::new("custody_watch_cleanup", cause))
            })?;
        }
        self.lease = None;
        Ok(())
    }
    fn check_proof(&self, proof: &BackendCustodyCleanup) -> Result<(), NativeObservationError> {
        let matches = if let Some(slot) = &self.slot {
            proof.matches_slot(slot)
        } else {
            self.metadata.as_ref().is_some_and(|metadata| proof.matches_metadata(metadata))
        };
        if matches { Ok(()) } else { Err(failure("custody_watch_cleanup_original")) }
    }
}
impl Drop for BackendCustodyProcessWatch {
    fn drop(&mut self) {
        if self.lease.is_some() {
            // Complete allocation/credit custody survives before diagnostics.
            std::mem::forget((
                self.lease.take(),
                self.wake.take(),
                self.signal.take(),
                self.slot.take(),
                self.metadata.take(),
            ));
            tracing::error!(
                operation = "custody_process_watch_abandoned",
                "Original native process/wait custody retained"
            );
        }
    }
}
fn failure(stage: &'static str) -> NativeObservationError {
    NativeObservationError::new(
        stage,
        std::io::Error::other("Custody process observation rejected"),
    )
}
