// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain every native output through actual attribute deletion and checked handle closure.
use super::*;
use crate::creation_error_mode::with_image_errors_reported;
use std::{
    mem::{size_of, size_of_val},
    os::windows::io::AsRawHandle,
};
use sugarsubstitute_execution::windows_fixture::FixtureForwardingEvents;
use windows::{
    Win32::{
        Foundation::{CloseHandle, FILETIME, HANDLE},
        System::{
            JobObjects::{
                IsProcessInJob, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JobObjectExtendedLimitInformation, QueryInformationJobObject,
            },
            Performance::{QueryPerformanceCounter, QueryPerformanceFrequency},
            Threading::*,
        },
    },
    core::{BOOL, PCWSTR, PWSTR},
};
#[path = "stdio.rs"]
mod stdio;
pub(super) struct NativeLease {
    process: Option<usize>,
    thread: Option<usize>,
    attributes: [usize; 64],
    initialized: bool,
    handles: [usize; 4],
    events: Option<FixtureForwardingEvents>,
    stdio: [Option<usize>; 3],
}
impl NativeLease {
    pub(super) fn new() -> Self {
        Self {
            process: None,
            thread: None,
            attributes: [0; 64],
            initialized: false,
            handles: [0; 4],
            events: None,
            stdio: [None; 3],
        }
    }
}
impl Lease for NativeLease {
    fn create(
        &mut self,
        input: &mut FixtureDescendantInput,
        context: &JobContext,
    ) -> Result<FixtureDescendantStarted, RegistryError> {
        if std::env::current_exe()
            .map_err(|cause| RegistryError::new("fixture_descendant_module", cause))?
            != input.module
        {
            return Err(RegistryError::invalid("fixture_descendant_module_changed"));
        }
        let mut member = BOOL::default();
        // SAFETY: borrowed current-process pseudo handle and initialized boolean output.
        unsafe { IsProcessInJob(GetCurrentProcess(), None, &mut member) }
            .map_err(|cause| RegistryError::new("fixture_root_containment", cause))?;
        native_checks::parent(member.as_bool(), 0)?;
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        // SAFETY: None queries this process's immediate Job; exact writable bounded structure.
        unsafe {
            QueryInformationJobObject(
                None,
                JobObjectExtendedLimitInformation,
                std::ptr::from_mut(&mut limits).cast(),
                size_of_val(&limits) as u32,
                None,
            )
        }
        .map_err(|cause| RegistryError::new("fixture_root_job_limits", cause))?;
        native_checks::parent(member.as_bool(), limits.BasicLimitInformation.LimitFlags.0)?;
        check_deadline(input.deadline)?;
        let handle_count = if input.output_mode {
            if input.events.is_some() {
                return Err(RegistryError::invalid("fixture_output_event_conflict"));
            }
            self.prepare_stdio()?;
            3
        } else {
            self.events = input.events.take();
            let view = self
                .events
                .as_ref()
                .ok_or_else(|| RegistryError::invalid("fixture_descendant_events"))?
                .borrow_inheritance();
            self.handles = [
                view.ready.as_raw_handle() as usize,
                view.release.as_raw_handle() as usize,
                view.challenge.as_raw_handle() as usize,
                view.response.as_raw_handle() as usize,
            ];
            4
        };
        let mut bytes = 0;
        // SAFETY: documented size query supplies exact required attribute-list bytes.
        if let Err(cause) = unsafe { InitializeProcThreadAttributeList(None, 1, None, &mut bytes) }
            && cause.code().0 as u32 != 0x8007007a
        {
            return Err(RegistryError::new("fixture_descendant_attribute_size", cause));
        }
        if bytes == 0 || bytes > size_of_val(&self.attributes) {
            return Err(RegistryError::invalid("fixture_descendant_attribute_size"));
        }
        let list = LPPROC_THREAD_ATTRIBUTE_LIST(self.attributes.as_mut_ptr().cast());
        // SAFETY: stable boxed NativeLease retains initialized aligned storage until deletion.
        unsafe { InitializeProcThreadAttributeList(Some(list), 1, None, &mut bytes) }
            .map_err(|cause| RegistryError::new("fixture_descendant_attributes", cause))?;
        self.initialized = true;
        // SAFETY: stable allowlist and event pin are retained through attribute deletion.
        unsafe {
            UpdateProcThreadAttribute(
                list,
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                Some(self.handles.as_ptr().cast()),
                handle_count * size_of::<usize>(),
                None,
                None,
            )
        }
        .map_err(|cause| RegistryError::new("fixture_descendant_allowlist", cause))?;
        let mut startup = STARTUPINFOEXW::default();
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup.lpAttributeList = list;
        if input.output_mode {
            startup.StartupInfo.dwFlags |= STARTF_USESTDHANDLES;
            startup.StartupInfo.hStdInput = HANDLE(self.handles[0] as *mut _);
            startup.StartupInfo.hStdOutput = HANDLE(self.handles[1] as *mut _);
            startup.StartupInfo.hStdError = HANDLE(self.handles[2] as *mut _);
        }
        let mut output = PROCESS_INFORMATION::default();
        let environment = [0_u16, 0];
        check_deadline(input.deadline)?;
        if context.cancellation_requested() {
            return Err(RegistryError::invalid("fixture_descendant_before_create"));
        }
        // A damaged or non-PE image must come back as the creation error, never as a modal dialog.
        let result = with_image_errors_reported(|| {
            // SAFETY: exact validated self-EXE/cwd/argv/empty environment remain live. Only the
            // selected four events or three stdio duplicates inherit. Parent Jobs are inherited.
            unsafe {
                CreateProcessW(
                    PCWSTR(input.command.application_name.as_ptr()),
                    Some(PWSTR(input.command.command_line.as_mut_ptr())),
                    None,
                    None,
                    true,
                    EXTENDED_STARTUPINFO_PRESENT
                        | CREATE_SUSPENDED
                        | CREATE_UNICODE_ENVIRONMENT
                        | CREATE_NO_WINDOW,
                    Some(environment.as_ptr().cast()),
                    PCWSTR(input.command.working_directory.as_ptr()),
                    &startup.StartupInfo,
                    &mut output,
                )
            }
        })
        .map_err(|cause| RegistryError::new("fixture_descendant_error_mode", cause))?;
        if !output.hProcess.is_invalid() {
            self.process = Some(output.hProcess.0 as usize);
        }
        if !output.hThread.is_invalid() {
            self.thread = Some(output.hThread.0 as usize);
        }
        result.map_err(|cause| RegistryError::new("fixture_descendant_create", cause))?;
        let process = owned(self.process)?;
        // SAFETY: actual created process output remains retained for every query.
        let process_id = unsafe { GetProcessId(process) };
        if process_id == 0 {
            return Err(RegistryError::new(
                "fixture_descendant_pid",
                std::io::Error::last_os_error(),
            ));
        }
        let (mut birth, mut exit, mut kernel, mut user) =
            (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        // SAFETY: retained exact process; all four outputs are initialized stack values.
        unsafe { GetProcessTimes(process, &mut birth, &mut exit, &mut kernel, &mut user) }
            .map_err(|cause| RegistryError::new("fixture_descendant_birth", cause))?;
        let creation_time_100ns =
            u64::from(birth.dwHighDateTime) << 32 | u64::from(birth.dwLowDateTime);
        if creation_time_100ns == 0 {
            return Err(RegistryError::invalid("fixture_descendant_birth"));
        }
        // SAFETY: query actual child's membership. Exact outer authority is later proven
        // by the outer registry's retained Job and complete PID+creation-time inventory.
        unsafe { IsProcessInJob(process, None, &mut member) }
            .map_err(|cause| RegistryError::new("fixture_descendant_containment", cause))?;
        if !member.as_bool() {
            return Err(RegistryError::invalid("fixture_descendant_uncontained"));
        }
        check_deadline(input.deadline)?;
        if context.cancellation_requested() {
            return Err(RegistryError::invalid("fixture_descendant_before_resume"));
        }
        // SAFETY: retained exact primary thread was created suspended with initial count1.
        let previous = unsafe { ResumeThread(owned(self.thread)?) };
        if previous == u32::MAX {
            return Err(RegistryError::new(
                "fixture_descendant_resume",
                std::io::Error::last_os_error(),
            ));
        }
        native_checks::resume(previous)?;
        Ok(FixtureDescendantStarted {
            correlation: input.correlation,
            process_id,
            creation_time_100ns,
        })
    }
    fn retire(&mut self) -> Result<(), RegistryError> {
        if self.initialized {
            // SAFETY: initialized list and pinned handle array remain live until this returns.
            unsafe {
                DeleteProcThreadAttributeList(LPPROC_THREAD_ATTRIBUTE_LIST(
                    self.attributes.as_mut_ptr().cast(),
                ))
            };
            self.initialized = false;
        }
        drop(self.events.take());
        self.handles = [0; 4];
        let mut failure: Option<RegistryError> = None;
        for slot in self.stdio.iter_mut().chain([&mut self.thread, &mut self.process]) {
            if let Some(raw) = *slot {
                // SAFETY: unique native output; failed closure leaves its exact slot retained.
                match unsafe { CloseHandle(HANDLE(raw as *mut _)) } {
                    Ok(()) => *slot = None,
                    Err(cause) => {
                        let error = RegistryError::new("fixture_descendant_close", cause);
                        if let Some(first) = &mut failure {
                            first.retain(error);
                        } else {
                            failure = Some(error);
                        }
                    }
                }
            }
        }
        failure.map_or(Ok(()), Err)
    }
}
fn owned(raw: Option<usize>) -> Result<HANDLE, RegistryError> {
    raw.map(|raw| HANDLE(raw as *mut _))
        .ok_or_else(|| RegistryError::invalid("fixture_descendant_output"))
}
fn check_deadline(
    deadline: sugarsubstitute_execution::windows_fixture::FixtureDeadline,
) -> Result<(), RegistryError> {
    let mut ticks = 0;
    let mut frequency = 0;
    // SAFETY: finite native clock query writes this initialized counter.
    unsafe { QueryPerformanceFrequency(&mut frequency) }
        .map_err(|cause| RegistryError::new("fixture_descendant_qpf", cause))?;
    // SAFETY: finite native clock query writes this initialized counter.
    unsafe { QueryPerformanceCounter(&mut ticks) }
        .map_err(|cause| RegistryError::new("fixture_descendant_qpc", cause))?;
    native_checks::deadline(deadline, ticks, frequency)
}
