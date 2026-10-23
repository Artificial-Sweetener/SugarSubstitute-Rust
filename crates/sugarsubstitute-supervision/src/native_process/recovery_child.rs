// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt a genuine inherited Job and checked-close only this carrier's own duplicates.
use super::*;
use crate::native_claims::files::{Budget, Handle};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_execution::{
    process_creation::{ProcessCreationMemory, ProcessCreationRetirement},
    windows_lifecycle_pipe::RecoveryDesktopBootstrap,
};
use windows::Win32::{
    Foundation::{GetHandleInformation, HANDLE_FLAG_INHERIT, SetHandleInformation},
    System::{
        Console::{GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE},
        JobObjects::{
            JOB_OBJECT_LIMIT_BREAKAWAY_OK, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JobObjectExtendedLimitInformation, QueryInformationJobObject,
        },
        Threading::GetCurrentProcess,
    },
};
impl Family {
    /// Adopt before every fallible status/security/membership observation under the genuine source.
    pub(crate) fn adopt_recovery_child_job(
        &mut self,
        raw: usize,
        memory: &ProcessCreationMemory,
        account: &mut Vec<Handle>,
        budget: Budget,
    ) -> Result<(), RegistryError> {
        if self.job.is_some() {
            return Err(RegistryError::invalid("recovery_child_job_reuse"));
        }
        self.job = Some(raw);
        self.job_list[0] = raw;
        let check = || {
            memory
                .check_effect()
                .map(|_| ())
                .map_err(|cause| RegistryError::new("recovery_child_job_control", cause))
        };
        check()?;
        let job = handle(self.job, "recovery_child_job")?;
        let stdio = original_stdio(&check)?;
        if stdio.contains(&raw) {
            return Err(RegistryError::invalid("recovery_child_job_stdio_alias"));
        }
        let mut flags = 0;
        // SAFETY: exact inherited slot is durably adopted before observing its native flags.
        unsafe { GetHandleInformation(job, &mut flags) }
            .map_err(|cause| RegistryError::new("recovery_child_job_flags", cause))?;
        if flags & HANDLE_FLAG_INHERIT.0 == 0 {
            return Err(RegistryError::invalid("recovery_child_job_not_inherited"));
        }
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        check()?;
        // SAFETY: caller-owned Job and initialized exact-sized limits structure remain live.
        unsafe {
            QueryInformationJobObject(
                Some(job),
                JobObjectExtendedLimitInformation,
                std::ptr::from_mut(&mut limits).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                None,
            )
        }
        .map_err(|cause| RegistryError::new("recovery_child_job_limits", cause))?;
        validate_containment(limits.BasicLimitInformation.LimitFlags.0)?;
        membership::verify_job_checked(unsafe_current_process(), job, &mut || check())?;
        let mut phase =
            crate::durable_storage::protected::native_phase::NativePhase::ordinary(&budget)
                .map_err(|cause| RegistryError::new("recovery_child_account_phase", cause))?;
        check()?;
        let actual = crate::durable_storage::protected::account::resolve_current(
            account,
            budget.clone(),
            crate::durable_storage::protected::account::PrimaryAccountAccess::Query,
            &mut phase,
        )
        .map_err(|cause| RegistryError::new("recovery_child_account", cause))?;
        check()?;
        crate::durable_storage::protected::security::verify_recovery_job_borrowed(
            job,
            &budget,
            actual.sid.bytes().map_err(|cause| RegistryError::new("recovery_child_sid", cause))?,
            &mut phase,
        )
        .map_err(|cause| RegistryError::new("recovery_child_job_security", cause))?;
        check()?;
        // SAFETY: narrow only this actual inherited handle; later UI HANDLE_LIST excludes the Job.
        unsafe {
            SetHandleInformation(
                job,
                HANDLE_FLAG_INHERIT.0,
                windows::Win32::Foundation::HANDLE_FLAGS(0),
            )
        }
        .map_err(|cause| RegistryError::new("recovery_child_job_narrow", cause))?;
        Ok(())
    }
    /// Borrow the genuine combined section/process pins and original stdio through attribute deletion.
    pub(crate) fn prepare_recovery_child_inheritance(
        &mut self,
        section: &RecoveryDesktopBootstrap,
    ) -> Result<(), RegistryError> {
        self.check_native()?;
        let view = section
            .borrow_inheritance()
            .map_err(|cause| RegistryError::new("recovery_child_inheritance", cause))?;
        let processes = view
            .capture_processes
            .ok_or_else(|| RegistryError::invalid("recovery_child_capture_processes"))?;
        let stdio = original_stdio(&|| self.check_native())?;
        let sources = [
            view.section.as_raw_handle() as usize,
            processes[0].as_raw_handle() as usize,
            processes[1].as_raw_handle() as usize,
        ];
        let (handles, count) = child_allowlist(
            sources,
            stdio,
            self.job.ok_or_else(|| RegistryError::invalid("recovery_child_job"))?,
        )?;
        self.inherited_handles = handles;
        self.inherited_count = count;
        self.stdio_handles = stdio;
        self.recovery_stdio = true;
        Ok(())
    }
    /// Close only inherited/local creation handles; never seal or terminate the shared profile Job.
    pub(crate) fn close_recovery_child(
        &mut self,
        proof: &ProcessCreationRetirement,
    ) -> Result<(), RegistryError> {
        proof.check().map_err(|cause| RegistryError::new("recovery_child_close_control", cause))?;
        WindowsPort.delete_attributes(self)?;
        for (slot, stage) in [
            (&mut self.thread, "recovery_child_close_thread"),
            (&mut self.watch_duplicate, "recovery_child_close_duplicate"),
            (&mut self.job, "recovery_child_close_job"),
        ] {
            proof
                .check()
                .map_err(|cause| RegistryError::new("recovery_child_close_control", cause))?;
            close_slot(slot, stage)?;
        }
        for member in self.members.iter_mut().flatten() {
            proof
                .check()
                .map_err(|cause| RegistryError::new("recovery_child_close_control", cause))?;
            close_slot(&mut member.handle, "recovery_child_close_process")?;
        }
        Ok(())
    }
}
/// Borrow the actual process pseudo handle without introducing close ownership.
fn unsafe_current_process() -> HANDLE {
    // SAFETY: the process pseudo handle is borrowed and never owned or closed.
    unsafe { GetCurrentProcess() }
}
/// Borrow the current carrier's three original stdio handles under unchanged effect controls.
fn original_stdio(
    check: &dyn Fn() -> Result<(), RegistryError>,
) -> Result<[usize; 3], RegistryError> {
    let mut values = [0; 3];
    for (slot, kind) in
        values.iter_mut().zip([STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE])
    {
        check()?;
        // SAFETY: each current-process standard handle is borrowed; no ownership is transferred.
        *slot = unsafe { GetStdHandle(kind) }
            .map_err(|cause| RegistryError::new("recovery_child_stdio", cause))?
            .0 as usize;
    }
    Ok(values)
}
/// Reject inherited Jobs that could release descendants outside original containment.
fn validate_containment(flags: u32) -> Result<(), RegistryError> {
    if flags & JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE.0 == 0
        || flags & (JOB_OBJECT_LIMIT_BREAKAWAY_OK | JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK).0 != 0
    {
        return Err(RegistryError::invalid("recovery_child_job_containment"));
    }
    Ok(())
}
/// Exclude the Job and every alias from the exact UI section/process/stdio allowlist.
fn child_allowlist(
    sources: [usize; 3],
    stdio: [usize; 3],
    job: usize,
) -> Result<([usize; 6], usize), RegistryError> {
    let values = [sources[0], sources[1], sources[2], stdio[0], stdio[1], stdio[2]];
    for (index, value) in values.iter().enumerate() {
        if *value == 0
            || *value >= usize::MAX - 15
            || *value == job
            || values[..index].contains(value)
        {
            return Err(RegistryError::invalid("recovery_child_inheritance_alias"));
        }
    }
    Ok((values, 6))
}
/// Check-close the one original inherited input only in its admitted finite cleanup job.
pub(crate) fn close_recovery_input(
    slot: &mut Option<usize>,
    context: &JobContext,
) -> Result<(), RegistryError> {
    if context.cancellation_requested()
        || context.deadline().is_none_or(|until| context.now() >= until)
    {
        return Err(RegistryError::invalid("recovery_child_input_close_expired"));
    }
    close_slot(slot, "recovery_child_input_close")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_child_requires_containment_and_rejects_breakaway() {
        assert!(validate_containment(JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE.0).is_ok());
        for flags in [
            0,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE.0 | JOB_OBJECT_LIMIT_BREAKAWAY_OK.0,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE.0 | JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK.0,
        ] {
            assert!(validate_containment(flags).is_err());
        }
    }
    #[test]
    fn recovery_child_ui_inherits_exact_six_handles_and_never_job() -> Result<(), RegistryError> {
        assert_eq!(child_allowlist([1, 2, 3], [4, 5, 6], 7)?, ([1, 2, 3, 4, 5, 6], 6));
        for (sources, stdio, job) in [
            ([1, 1, 3], [4, 5, 6], 7),
            ([1, 2, 3], [4, 4, 6], 7),
            ([1, 2, 3], [4, 5, 6], 1),
            ([1, 2, 3], [4, 5, 6], 4),
            ([1, 2, 3], [4, 5, 0], 7),
            ([1, 2, usize::MAX], [4, 5, 6], 7),
        ] {
            assert!(child_allowlist(sources, stdio, job).is_err());
        }
        Ok(())
    }
}
