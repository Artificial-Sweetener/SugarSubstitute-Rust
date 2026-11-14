// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe that a freshly started copy of the application shows its first window.
//!
//! A copy that restarts itself keeps its own waiting surface until the new copy's first window
//! is on screen. The observation is the real window: a visible top-level window with the
//! application's title, owned by another process created at or after the restart began. A
//! window of an older process, or of this one, never counts.
use crate::DesktopError;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use windows::Win32::{
    Foundation::*, System::SystemInformation::GetSystemTimeAsFileTime, System::Threading::*,
    UI::WindowsAndMessaging::*,
};

/// The current system time in 100-nanosecond units since 1601, the clock process creation
/// times use.
pub fn system_time_100ns() -> u64 {
    // SAFETY: a plain query of the system clock.
    let now = unsafe { GetSystemTimeAsFileTime() };
    filetime(now)
}

/// Whether a visible top-level window titled `title` belongs to a process other than this one
/// that was created at or after `since` (see [`system_time_100ns`]).
/// # Errors
/// Returns the window enumeration's failure.
pub fn application_window_shown_since(title: &str, since: u64) -> Result<bool, DesktopError> {
    let mut search = Search {
        title: title.encode_utf16().collect(),
        own: std::process::id(),
        since,
        found: false,
    };
    // SAFETY: EnumWindows invokes the callback synchronously with this live stack value. The
    // callback never retains pointers or unwinds across the ABI.
    unsafe { EnumWindows(Some(inspect), LPARAM((&raw mut search) as isize)) }
        .map_err(|cause| DesktopError::new("handoff_windows", cause))?;
    Ok(search.found)
}

struct Search {
    title: Vec<u16>,
    own: u32,
    since: u64,
    found: bool,
}

/// Borrow only the synchronous enumeration's stack state.
unsafe extern "system" fn inspect(window: HWND, context: LPARAM) -> windows::core::BOOL {
    // SAFETY: the only caller passes its live Search for this synchronous enumeration.
    let search = unsafe { &mut *(context.0 as *mut Search) };
    if search.found {
        return true.into();
    }
    let mut pid = 0;
    let mut title = [0u16; 64];
    // SAFETY: these queries accept transient HWNDs and retain no references.
    // InternalGetWindowText reads the stored title without sending a message, so a hung
    // window cannot block the enumeration.
    let titled = unsafe {
        GetWindowThreadProcessId(window, Some(&mut pid));
        if pid == 0 || pid == search.own || !IsWindowVisible(window).as_bool() {
            return true.into();
        }
        let length = usize::try_from(InternalGetWindowText(window, &mut title)).unwrap_or(0);
        title.get(..length) == Some(search.title.as_slice())
    };
    if titled && created(pid).is_some_and(|created| created >= search.since) {
        search.found = true;
    }
    // Stopping early would make EnumWindows report a failure with no cause.
    true.into()
}

/// The creation time of process `pid`, when it can still be opened.
fn created(pid: u32) -> Option<u64> {
    // SAFETY: a limited query handle, owned and closed once dropped.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    // SAFETY: the handle was just returned open and is owned only here.
    let process = unsafe { OwnedHandle::from_raw_handle(handle.0) };
    let (mut created, mut exited, mut kernel, mut user) =
        (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
    // SAFETY: the owned process and four writable FILETIME values live through the call.
    unsafe {
        GetProcessTimes(
            HANDLE(process.as_raw_handle()),
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        )
    }
    .ok()?;
    Some(filetime(created))
}

fn filetime(time: FILETIME) -> u64 {
    (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Windows of this process, and windows of processes created before the restart began,
    /// never count as the new copy's window.
    #[test]
    fn only_another_process_started_since_counts() -> Result<(), DesktopError> {
        let later = system_time_100ns().saturating_add(10_000_000_000);
        assert!(!application_window_shown_since("SugarSubstitute", later)?);
        Ok(())
    }
}
