// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exact native observations for a privately owned activation containment Job.
use super::*;
use std::mem::size_of;
use windows::{
    Win32::{
        Foundation::{WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::{JobObjects::*, Threading::*},
    },
    core::PCWSTR,
};

/// Own an unnamed, non-inheritable Job before enabling kill-on-close containment.
pub(super) fn job(budget: Budget) -> Result<Handle, ClaimError> {
    // SAFETY: unnamed non-inheritable output is adopted before subsequent fallible work.
    let raw = unsafe { CreateJobObjectW(None, PCWSTR::null()) }
        .map_err(|cause| ClaimError::new("activation_job_create", cause))?;
    let owned = Handle::new(raw, budget);
    limits(raw)?;
    Ok(owned)
}
/// Engine descendants remain kill-on-close contained for their entire lifetime.
fn limits(job: HANDLE) -> Result<(), ClaimError> {
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    // SAFETY: caller retains the exact private Job; the initialized structure has the API's size.
    unsafe {
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            std::ptr::from_ref(&limits).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    }
    .map_err(|cause| ClaimError::new("activation_job_limits", cause))
}
/// Resume exactly the primary thread created with one suspension after all outputs are adopted.
pub(super) fn resume(thread: HANDLE) -> Result<(), ClaimError> {
    // SAFETY: uniquely retained thread belongs to the just-created suspended engine.
    match unsafe { ResumeThread(thread) } {
        1 => Ok(()),
        u32::MAX => Err(ClaimError::new("activation_resume", windows::core::Error::from_thread())),
        _ => Err(ClaimError::invalid("activation_suspend_count")),
    }
}
/// Poll the retained kernel process without PID lookup or blocking the worker.
pub(super) fn exit(process: HANDLE) -> Result<Option<u32>, ClaimError> {
    // SAFETY: caller retains the actual process through this zero-time observation.
    match unsafe { WaitForSingleObject(process, 0) } {
        WAIT_TIMEOUT => Ok(None),
        WAIT_OBJECT_0 => {
            let mut code = 0;
            // SAFETY: the signaled process and writable status output remain live through the call.
            unsafe { GetExitCodeProcess(process, &mut code) }
                .map_err(|cause| ClaimError::new("activation_exit_code", cause))?;
            Ok(Some(code))
        }
        _ => Err(ClaimError::new("activation_exit_wait", windows::core::Error::from_thread())),
    }
}
/// Stop only the owned containment; successful termination request is not settlement evidence.
pub(super) fn terminate(job: HANDLE) -> Result<(), ClaimError> {
    // SAFETY: this private Job was associated atomically with only our engine and its descendants.
    unsafe { TerminateJobObject(job, 1) }
        .map_err(|cause| ClaimError::new("activation_terminate", cause))
}
/// Actual kernel membership reaching zero, not namespace absence, authorizes settled fallback.
pub(super) fn empty(job: HANDLE) -> Result<bool, ClaimError> {
    let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
    // SAFETY: live private Job and exactly sized initialized accounting output.
    unsafe {
        QueryInformationJobObject(
            Some(job),
            JobObjectBasicAccountingInformation,
            std::ptr::from_mut(&mut info).cast(),
            size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
            None,
        )
    }
    .map_err(|cause| ClaimError::new("activation_membership", cause))?;
    Ok(info.ActiveProcesses == 0)
}
