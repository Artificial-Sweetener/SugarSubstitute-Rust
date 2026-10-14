// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reuse the native Family under its original witness slot instead of claiming twice.
use super::*;
use crate::native_claims::backend_custody::pool::WitnessSlot;
use std::{mem::size_of_val, os::windows::io::IntoRawHandle};
use sugarsubstitute_execution::backend_custody::{BackendCustodyProcessWatch, BackendCustodyWake};
use windows::Win32::System::JobObjects::{
    JOB_OBJECT_LIMIT_BREAKAWAY_OK, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectExtendedLimitInformation, QueryInformationJobObject,
};

/// Keep the actual Family Box/data before the inseparable original512-KiB slot.
pub(crate) struct CustodyFamily {
    family: Option<FamilyStorage>,
    watches: Vec<Option<BackendCustodyProcessWatch>>,
    settled: bool,
    _slot: WitnessSlot,
}
impl CustodyFamily {
    /// Construct native storage before adopting any kernel output.
    pub(crate) fn new(slot: &WitnessSlot) -> Result<Self, RegistryError> {
        if slot.bytes() < crate::registry_input::FAMILY_STORAGE_BYTES {
            return Err(RegistryError::invalid("custody_family_funding"));
        }
        let mut watches = Vec::new();
        watches
            .try_reserve_exact(MAX_MEMBERS)
            .map_err(|cause| RegistryError::new("custody_watch_capacity", cause))?;
        watches.resize_with(MAX_MEMBERS, || None);
        let actual = crate::registry_input::FAMILY_STORAGE_BYTES as usize
            + MAX_MEMBERS * BackendCustodyProcessWatch::storage_bytes()
            + watches.capacity() * size_of::<Option<BackendCustodyProcessWatch>>()
            + size_of::<Self>();
        if actual > slot.bytes() as usize {
            return Err(RegistryError::invalid("custody_family_capacity"));
        }
        let mut family = Family::allocate(None)?;
        family.custody = Some(slot.clone());
        Ok(Self { family: Some(family), watches, settled: false, _slot: slot.clone() })
    }
    /// Adopt a transferred Job before querying its original containment contract.
    pub(crate) fn adopt_job(&mut self, input: OwnedHandle) -> Result<(), RegistryError> {
        let family =
            self.family.as_mut().ok_or_else(|| RegistryError::invalid("custody_family_closed"))?;
        if family.job.is_some() {
            return Err(RegistryError::invalid("custody_job_already_owned"));
        }
        family.job = Some(input.into_raw_handle() as usize);
        let job = handle(family.job, "custody_job")?;
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        family.check_native()?;
        // SAFETY: The exact transferred Job is durably adopted; output is initialized.
        unsafe {
            QueryInformationJobObject(
                Some(job),
                JobObjectExtendedLimitInformation,
                std::ptr::from_mut(&mut limits).cast(),
                size_of_val(&limits) as u32,
                None,
            )
        }
        .map_err(|cause| RegistryError::new("custody_job_limits", cause))?;
        let flags = limits.BasicLimitInformation.LimitFlags;
        if !flags.contains(JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE)
            || flags.0 & (JOB_OBJECT_LIMIT_BREAKAWAY_OK | JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK).0
                != 0
        {
            return Err(RegistryError::invalid("custody_job_containment"));
        }
        if !WindowsPort.inventory(family)?.empty {
            return Err(RegistryError::invalid("custody_job_not_prepared"));
        }
        Ok(())
    }
    /// Adopt the original suspended root before identity, membership or root ACK.
    pub(crate) fn adopt_root(
        &mut self,
        input: OwnedHandle,
        expected: Identity,
        wake: &BackendCustodyWake,
    ) -> Result<(), RegistryError> {
        let family =
            self.family.as_mut().ok_or_else(|| RegistryError::invalid("custody_family_closed"))?;
        if family.root.is_some() || family.count != 0 {
            return Err(RegistryError::invalid("custody_root_already_owned"));
        }
        let process = HANDLE(input.into_raw_handle());
        family.members[0] =
            Some(Member { handle: Some(process.0 as usize), identity: expected, exited: false });
        family.count = 1;
        let actual = membership::identity_checked(process, &mut || family.check_native())?;
        membership::verify_job_checked(
            process,
            handle(family.job, "custody_root_job")?,
            &mut || family.check_native(),
        )?;
        if actual != expected {
            return Err(RegistryError::invalid("custody_root_birth"));
        }
        family.root = Some(actual);
        WindowsPort.inventory(family)?;
        self.observe_members(wake, false)
    }
    /// Retain a distinct original wait for every actual bounded Job member.
    fn observe_members(
        &mut self,
        wake: &BackendCustodyWake,
        cleaning: bool,
    ) -> Result<(), RegistryError> {
        let family =
            self.family.as_mut().ok_or_else(|| RegistryError::invalid("custody_family_closed"))?;
        let turn =
            self._slot.turn().map_err(|cause| RegistryError::new("custody_member_turn", cause))?;
        for index in 0..family.count {
            if self.watches[index].is_some() {
                continue;
            }
            let identity = family.member(index)?.identity;
            let output = WindowsPort.duplicate(family, index)?;
            #[cfg(not(test))]
            let WatchInput::Native(input) = output;
            #[cfg(test)]
            let input = match output {
                WatchInput::Native(input) => input,
                #[cfg(test)]
                WatchInput::Simulated => {
                    return Err(RegistryError::invalid("custody_member_native"));
                }
            };
            self.watches[index] = Some(BackendCustodyProcessWatch::member(
                input,
                (identity.pid, identity.birth),
                turn,
            ));
            let watch = self.watches[index]
                .as_mut()
                .ok_or_else(|| RegistryError::invalid("custody_member_owner"))?;
            let started = if cleaning {
                let proof = self
                    ._slot
                    .cleanup_proof()
                    .map_err(|cause| RegistryError::new("custody_member_cleanup", cause))?;
                watch.activate_cleanup(wake.clone(), &proof)
            } else {
                watch.activate(wake.clone())
            };
            started.map_err(|cause| RegistryError::new("custody_member_observe", cause))?;
        }
        Ok(())
    }
    /// Read only the original root's actual callback signal, without PID probing.
    pub(crate) fn root_signaled(&self) -> bool {
        self.watches
            .first()
            .and_then(Option::as_ref)
            .is_some_and(BackendCustodyProcessWatch::signaled)
    }
    /// Advance checked same-family cleanup, preserving all pending outputs.
    pub(crate) fn retire(&mut self, wake: &BackendCustodyWake) -> Result<bool, RegistryError> {
        let family =
            self.family.as_mut().ok_or_else(|| RegistryError::invalid("custody_family_closed"))?;
        if self.settled {
            return Ok(true);
        }
        if family.incomplete {
            return Err(RegistryError::invalid("custody_inventory_uncertain"));
        }
        if !family.sealed {
            WindowsPort.seal(family)?;
        }
        if !family.forced {
            WindowsPort.terminate(family)?;
        }
        WindowsPort.inventory(family)?;
        self.observe_members(wake, true)?;
        let family =
            self.family.as_mut().ok_or_else(|| RegistryError::invalid("custody_family_closed"))?;
        let proof = self
            ._slot
            .cleanup_proof()
            .map_err(|cause| RegistryError::new("custody_family_cleanup", cause))?;
        for index in 0..family.count {
            if family.member(index)?.exited {
                continue;
            }
            let Some(watch) = self.watches[index].as_mut() else {
                return Err(RegistryError::invalid("custody_member_watch"));
            };
            if !watch.signaled() {
                return Ok(false);
            }
            watch
                .read_exit(&proof)
                .map_err(|cause| RegistryError::new("custody_member_exit", cause))?;
            let identity = family.member(index)?.identity;
            family.record_exit(identity)?;
        }
        if !family.all_exited() || !WindowsPort.inventory(family)?.empty {
            return Ok(false);
        }
        for watch in self.watches.iter_mut().flatten() {
            watch
                .retire(&proof)
                .map_err(|cause| RegistryError::new("custody_member_retire", cause))?;
        }
        self.watches.clear();
        WindowsPort.close(family)?;
        self.settled = true;
        Ok(true)
    }
}
impl Drop for CustodyFamily {
    fn drop(&mut self) {
        if let Some(family) = self.family.take() {
            if !self.settled && (family.job.is_some() || family.count != 0) {
                // Keep actual storage and original turn before diagnostics; no allocation/FFI.
                std::mem::forget((family, std::mem::take(&mut self.watches), self._slot.clone()));
                tracing::error!(
                    operation = "custody_family_abandoned",
                    "Actual unresolved family custody retained"
                );
            } else {
                drop(family);
            }
        }
    }
}
