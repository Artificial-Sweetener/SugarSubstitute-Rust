// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Create an explicit suspended child already associated with its owned Job.
use super::*;
use crate::creation_error_mode::with_image_errors_reported;
use std::{
    mem::{size_of, size_of_val},
    os::windows::io::FromRawHandle,
};
use windows::{
    Win32::{
        Foundation::{
            DUPLICATE_SAME_ACCESS, DuplicateHandle, ERROR_ALREADY_EXISTS, ERROR_SUCCESS,
            GetLastError, SetLastError,
        },
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
    /// Create the original empty containment Job before remote adoption or root creation.
    pub(super) fn prepare_job_native(&self, family: &mut Family) -> Result<(), RegistryError> {
        if family.job.is_some() || family.rejected_job.is_some() {
            return Err(RegistryError::invalid("prepared_job_already_owned"));
        }
        family.check_native()?;
        let name = family.cache_family.map(|identity| identity.native_name());
        let name_pointer = name.as_ref().map_or(PCWSTR::null(), |name| PCWSTR(name.as_ptr()));
        // SAFETY: the stable optional UTF-16 name lives through this synchronous creation.
        // Adopt even a rejected existing-object handle before any later fallible operation.
        let (created, existed) = unsafe {
            SetLastError(ERROR_SUCCESS);
            let created = CreateJobObjectW(None, name_pointer);
            let existed = GetLastError() == ERROR_ALREADY_EXISTS;
            (created, existed)
        };
        let job = created.map_err(|cause| RegistryError::new("create_job", cause))?;
        if existed {
            // This handle grants no ownership of the existing family. Cleanup closes only this
            // new handle; it must never change limits, enroll members or terminate that Job.
            family.rejected_job = Some(job.0 as usize);
            return Err(RegistryError::invalid("cache_containment_job_already_exists"));
        }
        family.job = Some(job.0 as usize);
        family.job_list[0] = job.0 as usize;
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        family.check_native()?;
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
        Ok(())
    }
    pub(super) fn create_native(
        &self,
        family: &mut Family,
        input: &mut WindowsLaunchInput,
    ) -> Result<Identity, RegistryError> {
        let job = handle(family.job, "prepared_creation_job")?;
        let inherited = family.inherited_count != 0;
        let attribute_count = if inherited { 2 } else { 1 };
        let mut bytes = 0;
        family.check_native()?;
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
        family.check_native()?;
        // SAFETY: preclaimed aligned storage covers queried bytes and remains durable until list deletion.
        unsafe { InitializeProcThreadAttributeList(Some(list), attribute_count, None, &mut bytes) }
            .map_err(|cause| RegistryError::new("initialize_attributes", cause))?;
        family.attributes_initialized = true;
        family.check_native()?;
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
        if inherited {
            family.check_native()?;
            // SAFETY: exactly the verified initialized prefix of one stable family
            // array is pinned through actual attribute deletion. No ambient handles.
            unsafe {
                UpdateProcThreadAttribute(
                    list,
                    0,
                    PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                    Some(family.inherited_handles.as_ptr().cast()),
                    family.inherited_count * size_of::<usize>(),
                    None,
                    None,
                )
            }
            .map_err(|cause| RegistryError::new("owned_handle_allowlist", cause))?;
        }
        let mut startup = STARTUPINFOEXW::default();
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup.lpAttributeList = list;
        family.configure_stdio(&mut startup.StartupInfo);
        let mut process = PROCESS_INFORMATION::default();
        family.check_native()?;
        // A damaged or non-PE image must come back as the creation error, never as a modal dialog.
        let created = with_image_errors_reported(|| {
            // SAFETY: Validated explicit argv, cwd and environment live through the call.
            // The exact HANDLE_LIST pins remain live through attribute deletion.
            unsafe {
                CreateProcessW(
                    PCWSTR(input.command.application_name.as_ptr()),
                    Some(PWSTR(input.command.command_line.as_mut_ptr())),
                    None,
                    None,
                    inherited,
                    creation_flags(family.backend_custody.is_some()),
                    Some(input.environment.as_utf16().as_ptr().cast()),
                    PCWSTR(input.command.working_directory.as_ptr()),
                    &startup.StartupInfo,
                    &mut process,
                )
            }
        })
        .map_err(|cause| RegistryError::new("creation_error_mode", cause))?;
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
        let identity = super::membership::identity_checked(
            handle(family.member(0)?.handle, "root_handle")?,
            &mut || family.check_native(),
        )?;
        super::membership::verify_job_checked(
            handle(family.member(0)?.handle, "root_handle")?,
            job,
            &mut || family.check_native(),
        )?;
        if let Some(member) = &mut family.members[0] {
            member.identity = identity;
        }
        family.root = Some(identity);
        Ok(identity)
    }
    pub(super) fn resume_native(&self, family: &mut Family) -> Result<(), RegistryError> {
        family.check_native()?;
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
        self.delete_attributes(family)?;
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
        family.check_native()?;
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
    pub(super) fn delete_attributes(&self, family: &mut Family) -> Result<(), RegistryError> {
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
        // the execution worker retire the pinned duplicated inheritance handles.
        family.release_inheritance()
    }
}

/// Console children start without a console window; Windows ignores this flag
/// for GUI images, preserving the desktop child's normal splash/Main visibility.
/// Custody alone retains the existing independent Job creation contract.
fn creation_flags(independent_custody: bool) -> PROCESS_CREATION_FLAGS {
    let flags = EXTENDED_STARTUPINFO_PRESENT
        | CREATE_SUSPENDED
        | CREATE_UNICODE_ENVIRONMENT
        | CREATE_NO_WINDOW;
    if independent_custody { flags | CREATE_BREAKAWAY_FROM_JOB } else { flags }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_native_launch_suppresses_console_creation_without_detaching_ownership() {
        let flags = creation_flags(false);
        assert!(flags.contains(CREATE_NO_WINDOW));
        assert!(flags.contains(CREATE_SUSPENDED | EXTENDED_STARTUPINFO_PRESENT));
        assert!(flags.contains(CREATE_UNICODE_ENVIRONMENT));
        assert_eq!(
            flags.0 & (CREATE_NEW_CONSOLE | DETACHED_PROCESS | CREATE_BREAKAWAY_FROM_JOB).0,
            0
        );
    }

    #[test]
    fn independent_custody_keeps_console_suppression_and_explicit_job_breakaway() {
        let flags = creation_flags(true);
        assert!(flags.contains(CREATE_NO_WINDOW | CREATE_BREAKAWAY_FROM_JOB));
        assert!(flags.contains(CREATE_SUSPENDED | EXTENDED_STARTUPINFO_PRESENT));
        assert!(flags.contains(CREATE_UNICODE_ENVIRONMENT));
        assert_eq!(flags.0 & (CREATE_NEW_CONSOLE | DETACHED_PROCESS).0, 0);
    }
}
