// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain exact process objects and request closure only for their own native windows.
use crate::{
    ClaimError,
    native_claims::files::{Budget, Handle},
};
use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf};
use windows::{
    Win32::{
        Foundation::*,
        System::{Diagnostics::ToolHelp::*, RestartManager::RM_UNIQUE_PROCESS, Threading::*},
        UI::WindowsAndMessaging::*,
    },
    core::PWSTR,
};

pub(super) struct Process {
    pub(super) handle: Handle,
    pub(super) pid: u32,
    pub(super) birth: u64,
    pub(super) image: PathBuf,
}
impl Process {
    /// Adopt the kernel handle before any fallible process observation.
    pub(super) fn open(
        pid: u32,
        terminate: bool,
        budget: Budget,
        outputs: &mut Vec<Self>,
    ) -> Result<usize, ClaimError> {
        if outputs.len() >= 128 {
            return Err(ClaimError::invalid("profile_conflict_family_bound"));
        }
        let rights = PROCESS_QUERY_LIMITED_INFORMATION
            | PROCESS_SYNCHRONIZE
            | if terminate { PROCESS_TERMINATE } else { PROCESS_ACCESS_RIGHTS(0) };
        // SAFETY: No inheritance or elevation is requested; the returned handle is adopted immediately.
        let raw = unsafe { OpenProcess(rights, false, pid) }
            .map_err(|cause| ClaimError::new("profile_conflict_process", cause))?;
        let index = outputs.len();
        outputs.push(Self {
            handle: Handle::new(raw, budget),
            pid,
            birth: 0,
            image: PathBuf::new(),
        });
        let process = &mut outputs[index];
        let (mut birth, mut exit, mut kernel, mut user) =
            (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        // SAFETY: The retained process and initialized time outputs live through the query.
        unsafe { GetProcessTimes(raw, &mut birth, &mut exit, &mut kernel, &mut user) }
            .map_err(|cause| ClaimError::new("profile_conflict_birth", cause))?;
        process.birth = ticks(birth);
        let mut name = vec![0; 32768];
        let mut length = name.len() as u32;
        // SAFETY: The buffer's advertised capacity matches its initialized UTF-16 storage.
        unsafe {
            QueryFullProcessImageNameW(
                raw,
                PROCESS_NAME_WIN32,
                PWSTR(name.as_mut_ptr()),
                &mut length,
            )
        }
        .map_err(|cause| ClaimError::new("profile_conflict_image", cause))?;
        process.image = OsString::from_wide(&name[..length as usize]).into();
        Ok(index)
    }
    pub(super) fn matches(&self, candidate: &RM_UNIQUE_PROCESS) -> bool {
        matches_identity(self.pid, self.birth, candidate)
    }
    pub(super) fn exited(&self) -> Result<bool, ClaimError> {
        // SAFETY: A zero-time query observes the retained object without blocking.
        match unsafe { WaitForSingleObject(self.handle.raw(), 0) } {
            WAIT_OBJECT_0 => Ok(true),
            WAIT_TIMEOUT => Ok(false),
            _ => Err(ClaimError::new("profile_conflict_wait", windows::core::Error::from_thread())),
        }
    }
    pub(super) fn force(&self) -> Result<(), ClaimError> {
        if self.exited()? {
            return Ok(());
        }
        // SAFETY: This is the retained, verified coordinator object; no PID lookup occurs here.
        unsafe { TerminateProcess(self.handle.raw(), 1) }
            .map_err(|cause| ClaimError::new("profile_conflict_force", cause))
    }
}
pub(super) fn ticks(value: FILETIME) -> u64 {
    (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime)
}
fn matches_identity(pid: u32, birth: u64, candidate: &RM_UNIQUE_PROCESS) -> bool {
    pid == candidate.dwProcessId && birth == ticks(candidate.ProcessStartTime)
}

/// Capture only descendants of already retained original objects, bounded to 128 processes.
pub(super) fn family(
    root: usize,
    processes: &mut Vec<Process>,
    budget: Budget,
    handles: &mut Vec<Handle>,
) -> Result<(), ClaimError> {
    // SAFETY: Snapshot has no process mutation rights and is adopted before reading.
    let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }
        .map_err(|cause| ClaimError::new("profile_conflict_snapshot", cause))?;
    handles.push(Handle::new(raw, budget.clone()));
    let mut row =
        PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
    let mut entries = Vec::new();
    // SAFETY: Snapshot and initialized exact-sized entry remain live for each query.
    let mut next = unsafe { Process32FirstW(raw, &mut row) };
    loop {
        match next {
            Ok(()) => {}
            Err(cause) if cause.code() == ERROR_NO_MORE_FILES.to_hresult() => break,
            Err(cause) => return Err(ClaimError::new("profile_conflict_snapshot_read", cause)),
        }
        if entries.len() >= 16384 {
            return Err(ClaimError::invalid("profile_conflict_snapshot_bound"));
        }
        entries.push((row.th32ProcessID, row.th32ParentProcessID));
        // SAFETY: Same initialized entry and retained snapshot as above.
        next = unsafe { Process32NextW(raw, &mut row) };
    }
    let mut cursor = root;
    while cursor < processes.len() {
        let (parent, born) = (processes[cursor].pid, processes[cursor].birth);
        for &(pid, parent_pid) in &entries {
            if parent_pid != parent || processes.iter().any(|process| process.pid == pid) {
                continue;
            }
            match Process::open(pid, false, budget.clone(), processes) {
                Ok(index) => {
                    if processes[index].birth < born {
                        processes[index].handle.close()?;
                        processes.pop();
                    }
                }
                Err(cause) if cause.win32_code() == Some(ERROR_INVALID_PARAMETER.0) => {}
                Err(cause) => return Err(cause),
            }
        }
        cursor += 1;
    }
    Ok(())
}

/// Post normal close to top-level product windows belonging to retained family objects.
pub(super) fn close_windows(
    processes: &[Process],
    image: &std::path::Path,
) -> Result<(), ClaimError> {
    struct Selection<'a> {
        processes: &'a [Process],
        image: &'a std::path::Path,
        failure: Option<ClaimError>,
    }
    unsafe extern "system" fn visit(hwnd: HWND, data: LPARAM) -> windows::core::BOOL {
        // SAFETY: EnumWindows synchronously borrows the live Selection and never retains it.
        let state = unsafe { &mut *(data.0 as *mut Selection<'_>) };
        let mut pid = 0;
        // SAFETY: HWND queries tolerate disappearance; posting has no synchronous target callback.
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if state
                .processes
                .iter()
                .any(|process| process.pid == pid && process.image == state.image)
                && GetAncestor(hwnd, GA_ROOTOWNER) == hwnd
                && GetWindowTextLengthW(hwnd) > 0
                && GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 == 0
                && let Err(cause) = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0))
            {
                state.failure = Some(ClaimError::new("profile_conflict_close_window", cause));
            }
        }
        true.into()
    }
    let mut selection = Selection { processes, image, failure: None };
    // SAFETY: The callback's borrowed state remains live throughout synchronous enumeration.
    unsafe { EnumWindows(Some(visit), LPARAM((&raw mut selection) as isize)) }
        .map_err(|cause| ClaimError::new("profile_conflict_windows", cause))?;
    selection.failure.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reused_pid_and_foreign_process_cannot_match_a_retained_owner() {
        let original = RM_UNIQUE_PROCESS {
            dwProcessId: 42,
            ProcessStartTime: FILETIME { dwLowDateTime: 7, dwHighDateTime: 3 },
        };
        let birth = (3_u64 << 32) | 7;
        assert!(matches_identity(42, birth, &original));
        assert!(!matches_identity(42, birth + 1, &original));
        assert!(!matches_identity(43, birth, &original));
        assert!(!matches_identity(42, 7, &original));
    }
}
