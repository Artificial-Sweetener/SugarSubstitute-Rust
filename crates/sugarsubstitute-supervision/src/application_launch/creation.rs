// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Explicit inheritance for an independent application Root, with owned attribute storage.
use super::LaunchCustody;
use crate::{ClaimError, WindowsLaunchBuffers};
use std::{mem::size_of, os::windows::io::AsRawHandle};
use windows::{
    Win32::System::Threading::*,
    core::{PCWSTR, PWSTR},
};

struct Attributes {
    storage: Box<[usize; 128]>,
    initialized: bool,
}
impl Attributes {
    fn pointer(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        LPPROC_THREAD_ATTRIBUTE_LIST(self.storage.as_mut_ptr().cast())
    }
    fn initialize(&mut self, handles: &[usize]) -> Result<(), ClaimError> {
        let mut bytes = 0;
        // SAFETY: documented sizing query supplies required bytes without accessing a list.
        if let Err(cause) = unsafe { InitializeProcThreadAttributeList(None, 1, None, &mut bytes) }
            && cause.code().0 as u32 != 0x8007007a
        {
            return Err(ClaimError::new("application_launch_attributes_size", cause));
        }
        if bytes == 0 || bytes > size_of::<[usize; 128]>() {
            return Err(ClaimError::invalid("application_launch_attributes_capacity"));
        }
        let list = self.pointer();
        // SAFETY: stable aligned heap storage covers the measured list size through Drop.
        unsafe { InitializeProcThreadAttributeList(Some(list), 1, None, &mut bytes) }
            .map_err(|cause| ClaimError::new("application_launch_attributes", cause))?;
        self.initialized = true;
        // SAFETY: the caller retains these bounded handle values and their owning custody throughout
        // process creation and attribute deletion. No ambient handle is added to the list.
        unsafe {
            UpdateProcThreadAttribute(
                list,
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                Some(handles.as_ptr().cast()),
                std::mem::size_of_val(handles),
                None,
                None,
            )
        }
        .map_err(|cause| ClaimError::new("application_launch_inheritance", cause))
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        if self.initialized {
            // SAFETY: this uniquely owned list remains in its original heap allocation.
            unsafe { DeleteProcThreadAttributeList(self.pointer()) };
        }
    }
}

/// Return native status separately from outputs so the caller adopts every produced handle.
pub(super) fn create(
    command: &mut WindowsLaunchBuffers,
    custody: Option<&LaunchCustody>,
    process: &mut PROCESS_INFORMATION,
) -> Result<(), ClaimError> {
    let (handles, count) = match custody {
        Some(LaunchCustody::Selected(selection)) => {
            ([selection.selection_handle().as_raw_handle() as usize, 0], 1)
        }
        Some(LaunchCustody::Trial(trial)) => {
            (trial.transfer()?.handles().map(|handle| handle.as_raw_handle() as usize), 2)
        }
        None => ([0; 2], 0),
    };
    let inherited = &handles[..count];
    let mut attributes = Attributes { storage: Box::new([0; 128]), initialized: false };
    let mut startup = STARTUPINFOEXW::default();
    // Windows ignores CREATE_NO_WINDOW for GUI images; their splash/Main remains visible.
    // https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags
    let mut flags = CREATE_UNICODE_ENVIRONMENT | CREATE_NO_WINDOW;
    if !inherited.is_empty() {
        attributes.initialize(inherited)?;
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup.lpAttributeList = attributes.pointer();
        flags |= EXTENDED_STARTUPINFO_PRESENT;
    } else {
        startup.StartupInfo.cb = size_of::<STARTUPINFOW>() as u32;
    }
    // SAFETY: exact validated command buffers, current-directory buffer, original inherited
    // selection and heap attribute list remain live through this call. A new independent Root
    // owns its lifetime from successful creation; it is not put in the launcher's shutdown Job.
    unsafe {
        CreateProcessW(
            PCWSTR(command.application_name.as_ptr()),
            Some(PWSTR(command.command_line.as_mut_ptr())),
            None,
            None,
            !inherited.is_empty(),
            flags,
            None,
            PCWSTR(command.working_directory.as_ptr()),
            &startup.StartupInfo,
            process,
        )
    }
    .map_err(|cause| ClaimError::new("application_launch_create", cause))
}
