// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Explicit inheritance for an independent application Root, with owned attribute storage.
use super::LaunchCustody;
use crate::{ClaimError, WindowsLaunchBuffers, creation_error_mode::with_image_errors_reported};
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
        let count = 1;
        let mut bytes = 0;
        // SAFETY: documented sizing query supplies required bytes without accessing a list.
        let measured = unsafe { InitializeProcThreadAttributeList(None, count, None, &mut bytes) };
        if let Err(cause) = measured
            && cause.code().0 as u32 != 0x8007007a
        {
            return Err(ClaimError::new("application_launch_attributes_size", cause));
        }
        if bytes == 0 || bytes > size_of::<[usize; 128]>() {
            return Err(ClaimError::invalid("application_launch_attributes_capacity"));
        }
        let list = self.pointer();
        // SAFETY: stable aligned heap storage covers the measured list size through Drop.
        unsafe { InitializeProcThreadAttributeList(Some(list), count, None, &mut bytes) }
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
        .map_err(|cause| ClaimError::new("application_launch_inheritance", cause))?;
        Ok(())
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
            ([selection.selection_handle().as_raw_handle() as usize, 0, 0], 1)
        }
        Some(LaunchCustody::Trial(trial)) => {
            (trial.transfer()?.handles().map(|handle| handle.as_raw_handle() as usize), 3)
        }
        None => ([0; 3], 0),
    };
    create_native(command, &handles[..count], process)
}

/// Create a generation's launch engine, uncontained, inheriting exactly its takeover event.
pub(super) fn engine(
    command: &mut WindowsLaunchBuffers,
    event: usize,
    process: &mut PROCESS_INFORMATION,
) -> Result<(), ClaimError> {
    create_native(command, std::slice::from_ref(&event), process)
}

/// Keep attribute storage, exact inheritance and native outputs under one creation implementation.
fn create_native(
    command: &mut WindowsLaunchBuffers,
    inherited: &[usize],
    process: &mut PROCESS_INFORMATION,
) -> Result<(), ClaimError> {
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
    // A damaged or non-PE image must come back as the creation error, never as a modal dialog.
    with_image_errors_reported(|| {
        // SAFETY: exact validated command buffers, current-directory buffer, original inherited
        // handles and heap attribute list remain live through this call. The caller adopts every
        // output.
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
    })
    .map_err(|cause| ClaimError::new("application_launch_error_mode", cause))?
    .map_err(|cause| ClaimError::new("application_launch_create", cause))
}
