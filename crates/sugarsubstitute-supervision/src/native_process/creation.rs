// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Create an explicit suspended child already associated with its owned Job.
use super::*;
use std::{
    mem::{size_of, size_of_val},
    os::windows::io::FromRawHandle,
};
use windows::{
    Win32::{
        Foundation::{DUPLICATE_SAME_ACCESS, DuplicateHandle},
        System::{
            JobObjects::{
                CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
                SetInformationJobObject,
            },
            Threading::*,
        },
    },
    core::{PCWSTR, PWSTR},
};

impl WindowsPort {
    pub(super) fn create_native(
        &self,
        family: &mut Family,
        input: &mut WindowsLaunchInput,
    ) -> Result<Identity, RegistryError> {
        // SAFETY: unnamed, non-inheritable Job is immediately adopted before any later fallible operation.
        let job = unsafe { CreateJobObjectW(None, PCWSTR::null()) }
            .map_err(|cause| RegistryError::new("create_job", cause))?;
        family.job = Some(job.0 as usize);
        family.job_list[0] = job.0 as usize;
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: exact owned Job and initialized structure; no ordinary process-count limit or breakaway is set.
        unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                std::ptr::from_ref(&limits).cast(),
                size_of_val(&limits) as u32,
            )
        }
        .map_err(|cause| RegistryError::new("job_containment_limits", cause))?;
        let attribute_count = if family.fixture_events.is_some() { 2 } else { 1 };
        let mut bytes = 0;
        // SAFETY: sizing query has no list; the documented insufficient-buffer result supplies its required size.
        let sized =
            unsafe { InitializeProcThreadAttributeList(None, attribute_count, None, &mut bytes) };
        if let Err(cause) = sized
            && cause.code().0 as u32 != 0x8007007a
        {
            return Err(RegistryError::new("attribute_size", cause));
        }
        if bytes == 0 || bytes > family.attributes.len() * size_of::<usize>() {
            return Err(RegistryError::invalid("attribute_size"));
        }
        let list = LPPROC_THREAD_ATTRIBUTE_LIST(family.attributes.as_mut_ptr().cast());
        // SAFETY: preclaimed aligned storage covers queried bytes and remains durable until list deletion.
        unsafe { InitializeProcThreadAttributeList(Some(list), attribute_count, None, &mut bytes) }
            .map_err(|cause| RegistryError::new("initialize_attributes", cause))?;
        family.attributes_initialized = true;
        // SAFETY: stable boxed family owns the one Job handle value through DeleteProcThreadAttributeList.
        unsafe {
            UpdateProcThreadAttribute(
                list,
                0,
                PROC_THREAD_ATTRIBUTE_JOB_LIST as usize,
                Some(family.job_list.as_ptr().cast()),
                size_of::<usize>(),
                None,
                None,
            )
        }
        .map_err(|cause| RegistryError::new("job_at_creation_attribute", cause))?;
        if family.fixture_events.is_some() {
            // SAFETY: the durable boxed family and session pin keep exactly four
            // inheritable event duplicates live through actual attribute deletion.
            unsafe {
                UpdateProcThreadAttribute(
                    list,
                    0,
                    PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                    Some(family.fixture_handles.as_ptr().cast()),
                    size_of_val(&family.fixture_handles),
                    None,
                    None,
                )
            }
            .map_err(|cause| RegistryError::new("fixture_handle_allowlist", cause))?;
        }
        let mut startup = STARTUPINFOEXW::default();
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup.lpAttributeList = list;
        let mut process = PROCESS_INFORMATION::default();
        // SAFETY: validated explicit mutable argv, application/cwd/environment live through the call.
        // Ordinary launches inherit no handles. The fixture inherits only its exact
        // four-event HANDLE_LIST; the owned Job and process handles remain private.
        let created = unsafe {
            CreateProcessW(
                PCWSTR(input.command.application_name.as_ptr()),
                Some(PWSTR(input.command.command_line.as_mut_ptr())),
                None,
                None,
                family.fixture_events.is_some(),
                EXTENDED_STARTUPINFO_PRESENT | CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT,
                Some(input.environment.as_utf16().as_ptr().cast()),
                PCWSTR(input.command.working_directory.as_ptr()),
                &startup.StartupInfo,
                &mut process,
            )
        };
        if !process.hProcess.is_invalid() {
            family.members[0] = Some(Member {
                handle: Some(process.hProcess.0 as usize),
                identity: Identity { pid: 0, birth: 0 },
                exited: false,
            });
            family.count = 1;
        }
        if !process.hThread.is_invalid() {
            family.thread = Some(process.hThread.0 as usize);
        }
        created.map_err(|cause| RegistryError::new("create_process", cause))?;
        let identity =
            super::membership::identity(handle(family.member(0)?.handle, "root_handle")?)?;
        super::membership::verify_job(handle(family.member(0)?.handle, "root_handle")?, job)?;
        if let Some(member) = &mut family.members[0] {
            member.identity = identity;
        }
        family.root = Some(identity);
        Ok(identity)
    }
    pub(super) fn resume_native(&self, family: &mut Family) -> Result<(), RegistryError> {
        // SAFETY: exact primary thread was created suspended and is retained across failure.
        let previous = unsafe { ResumeThread(handle(family.thread, "primary_thread")?) };
        if previous == u32::MAX {
            return Err(RegistryError::new(
                "resume_primary_thread",
                std::io::Error::last_os_error(),
            ));
        }
        if previous == 1 {
            family.resumed = true;
        }
        if previous != 1 {
            return Err(RegistryError::invalid("primary_suspend_count"));
        }
        close_slot(&mut family.thread, "close_primary_thread")?;
        self.delete_attributes(family);
        Ok(())
    }
    pub(super) fn duplicate_native(
        &self,
        family: &mut Family,
        index: usize,
    ) -> Result<WatchInput, RegistryError> {
        if family.watch_duplicate.is_some() {
            return Err(RegistryError::invalid("duplicate_watch_retirement"));
        }
        let original = handle(family.member(index)?.handle, "member_handle")?;
        let mut duplicate = HANDLE::default();
        // SAFETY: original remains retained, current process pseudo handles are borrowed;
        // every noninvalid output is adopted even if native duplication reports failure.
        let copied = unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                original,
                GetCurrentProcess(),
                &mut duplicate,
                0,
                false,
                DUPLICATE_SAME_ACCESS,
            )
        };
        if duplicate.is_invalid() {
            copied.map_err(|cause| RegistryError::new("duplicate_watch_process", cause))?;
            return Err(RegistryError::invalid("duplicate_watch_process"));
        }
        family.watch_duplicate = Some(duplicate.0 as usize);
        copied.map_err(|cause| RegistryError::new("duplicate_watch_process", cause))?;
        // SAFETY: success transfers the exact durable handle slot to one owned observation input.
        let owned = unsafe { OwnedHandle::from_raw_handle(duplicate.0) };
        family.watch_duplicate = None;
        Ok(WatchInput::Native(owned))
    }
    pub(super) fn delete_attributes(&self, family: &mut Family) {
        if family.attributes_initialized {
            // SAFETY: initialized list and all value storage remain retained; deletion cannot fail.
            unsafe {
                DeleteProcThreadAttributeList(LPPROC_THREAD_ATTRIBUTE_LIST(
                    family.attributes.as_mut_ptr().cast(),
                ))
            };
            family.attributes_initialized = false;
        }
        // Deletion has returned, or initialization never committed. Only now can
        // the execution worker retire the duplicated event handles.
        drop(family.fixture_events.take());
        family.fixture_handles = [0; 4];
    }
}
