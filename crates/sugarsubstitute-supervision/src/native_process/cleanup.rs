// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Seal creation, retain every limit, verify seal readback and close actual owned handles.
use super::*;
use std::mem::size_of_val;
use windows::Win32::System::JobObjects::{
    JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
    TerminateJobObject,
};

impl WindowsPort {
    fn seal_native(&self, family: &mut Family) -> Result<(), RegistryError> {
        let job = handle(family.job, "seal_job")?;
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        // SAFETY: exact live Job handle and complete initialized native output.
        unsafe {
            QueryInformationJobObject(
                Some(job),
                JobObjectExtendedLimitInformation,
                std::ptr::from_mut(&mut limits).cast(),
                size_of_val(&limits) as u32,
                None,
            )
        }
        .map_err(|cause| RegistryError::new("query_job_limits", cause))?;
        limits.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
        limits.BasicLimitInformation.ActiveProcessLimit = 0;
        // SAFETY: all previously queried fields/flags are preserved except the explicit seal.
        unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                std::ptr::from_ref(&limits).cast(),
                size_of_val(&limits) as u32,
            )
        }
        .map_err(|cause| RegistryError::new("seal_job_limits", cause))?;
        let mut verified = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        // SAFETY: readback uses the same exact live Job and a complete output structure.
        unsafe {
            QueryInformationJobObject(
                Some(job),
                JobObjectExtendedLimitInformation,
                std::ptr::from_mut(&mut verified).cast(),
                size_of_val(&verified) as u32,
                None,
            )
        }
        .map_err(|cause| RegistryError::new("readback_job_seal", cause))?;
        if !verified.BasicLimitInformation.LimitFlags.contains(JOB_OBJECT_LIMIT_ACTIVE_PROCESS)
            || verified.BasicLimitInformation.ActiveProcessLimit != 0
            || verified.BasicLimitInformation.LimitFlags != limits.BasicLimitInformation.LimitFlags
        {
            return Err(RegistryError::invalid("readback_job_seal"));
        }
        family.sealed = true;
        Ok(())
    }
    fn close_native(&self, family: &mut Family) -> Result<(), RegistryError> {
        self.delete_attributes(family)?;
        let mut failure = None;
        let mut keep = |next: Result<(), RegistryError>| {
            if let Err(cause) = next {
                match &mut failure {
                    Some(first) => RegistryError::retain(first, cause),
                    None => failure = Some(cause),
                }
            }
        };
        // Closing the Job first is the authorized owned-family owner-loss safety boundary.
        // During normal settlement complete empty membership was already established.
        keep(close_slot(&mut family.job, "close_job"));
        keep(close_slot(&mut family.thread, "close_primary_thread"));
        keep(close_slot(&mut family.watch_duplicate, "close_watch_duplicate"));
        drop(family.lifecycle_enrollment.take());
        for member in family.members.iter_mut().flatten() {
            keep(close_slot(&mut member.handle, "close_member_process"));
        }
        match failure {
            Some(cause) => Err(cause),
            None => Ok(()),
        }
    }
}
impl NativePort for WindowsPort {
    fn create(
        &self,
        family: &mut Family,
        input: &mut WindowsLaunchInput,
    ) -> Result<Identity, RegistryError> {
        self.create_native(family, input)
    }
    fn resume(&self, family: &mut Family) -> Result<(), RegistryError> {
        self.resume_native(family)
    }
    fn duplicate(&self, family: &mut Family, index: usize) -> Result<WatchInput, RegistryError> {
        self.duplicate_native(family, index)
    }
    fn enroll_desktop(
        &self,
        family: &mut Family,
        run: RunKey,
        key: ProcessKey,
    ) -> Result<(), RegistryError> {
        self.enroll_native(family, run, key)
    }
    fn seal(&self, family: &mut Family) -> Result<(), RegistryError> {
        let result = self.seal_native(family);
        if result.is_err() {
            family.incomplete = true;
        }
        result
    }
    fn inventory(&self, family: &mut Family) -> Result<Inventory, RegistryError> {
        let result = self.inventory_native(family);
        if result.is_err() {
            family.incomplete = true;
        }
        result
    }
    fn terminate(&self, family: &mut Family) -> Result<(), RegistryError> {
        // SAFETY: registry owns this exact Job and stop was authorized; no PID/global lookup.
        unsafe { TerminateJobObject(handle(family.job, "terminate_job")?, 1) }
            .map_err(|cause| RegistryError::new("terminate_job", cause))?;
        family.forced = true;
        Ok(())
    }
    fn close(&self, family: &mut Family) -> Result<(), RegistryError> {
        self.close_native(family)
    }
}
