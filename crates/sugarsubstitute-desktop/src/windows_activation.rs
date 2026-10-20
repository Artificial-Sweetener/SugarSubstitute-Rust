// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Restore an authenticated desktop process's existing top-level window.
use crate::DesktopError;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use windows::Win32::{Foundation::*, System::Threading::*, UI::WindowsAndMessaging::*};

/// Restore a running desktop after a repeated launch, without waiting on its UI thread.
/// The caller supplies the PID and creation time from its authenticated desktop channel.
/// Returns false after process exit or before a top-level application window exists.
/// Foreground selection remains subject to Windows focus policy; a denied focus request
/// still restores the visible window. Native query failures preserve their cause.
pub fn activate_desktop_process(pid: u32, creation_time_100ns: u64) -> Result<bool, DesktopError> {
    // SAFETY: this query grants no process mutation rights. The returned handle is
    // adopted immediately and remains owned through identity and window selection.
    let raw = match unsafe {
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE, false, pid)
    } {
        Ok(raw) => raw,
        Err(cause)
            if cause.code() == windows::core::HRESULT::from_win32(ERROR_INVALID_PARAMETER.0) =>
        {
            return Ok(false);
        }
        Err(cause) => return Err(DesktopError::new("activate_desktop_process", cause)),
    };
    // SAFETY: OpenProcess returned this uniquely owned kernel handle.
    let process = unsafe { OwnedHandle::from_raw_handle(raw.0) };
    let handle = HANDLE(process.as_raw_handle());
    let (mut created, mut exited, mut kernel, mut user) =
        (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
    // SAFETY: the owned process and four writable FILETIME values live through the call.
    unsafe { GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) }
        .map_err(|cause| DesktopError::new("activate_desktop_identity", cause))?;
    let birth = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
    // SAFETY: a zero-duration query never blocks the caller or target thread.
    if birth != creation_time_100ns || unsafe { WaitForSingleObject(handle, 0) } != WAIT_TIMEOUT {
        return Ok(false);
    }
    restore_window(pid)
}

/// Select one live top-level surface without exposing a preparing hidden window.
fn restore_window(pid: u32) -> Result<bool, DesktopError> {
    let mut selection = Selection { pid, visible: None, hidden: None };
    // SAFETY: EnumWindows invokes the callback synchronously with this live stack
    // value. The callback never retains pointers or unwinds across the ABI.
    unsafe { EnumWindows(Some(select), LPARAM((&raw mut selection) as isize)) }
        .map_err(|cause| DesktopError::new("activate_desktop_windows", cause))?;
    let Some(window) = selection.visible.or(selection.hidden) else {
        return Ok(false);
    };
    let mut actual_pid = 0;
    // SAFETY: HWNDs are queried again before use; disappearance is a benign false result.
    unsafe { GetWindowThreadProcessId(window, Some(&mut actual_pid)) };
    if actual_pid != pid {
        return Ok(false);
    }
    // SAFETY: these nonblocking operations target only the selected application's
    // top-level window. Windows marshals ShowWindowAsync onto its owning UI thread.
    unsafe {
        let show = if IsIconic(window).as_bool() { SW_RESTORE } else { SW_SHOW };
        if !ShowWindowAsync(window, show).as_bool() {
            return Ok(false);
        }
        let foreground = SetForegroundWindow(window).as_bool();
        tracing::info!(
            operation = "desktop_reactivated",
            process_id = pid,
            foreground,
            "Restored the existing application window after a repeated launch"
        );
    }
    Ok(true)
}

struct Selection {
    pid: u32,
    visible: Option<HWND>,
    hidden: Option<HWND>,
}

/// Borrow only the synchronous enumeration's stack state; inspect no other process data.
unsafe extern "system" fn select(window: HWND, context: LPARAM) -> windows::core::BOOL {
    // SAFETY: the only caller passes its live Selection for this synchronous enumeration.
    let selection = unsafe { &mut *(context.0 as *mut Selection) };
    let mut pid = 0;
    // SAFETY: these queries accept transient HWNDs and retain no references.
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut pid));
        if pid == selection.pid
            && GetAncestor(window, GA_ROOTOWNER) == window
            && GetWindowLongPtrW(window, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 == 0
            && GetWindowTextLengthW(window) > 0
        {
            if IsWindowVisible(window).as_bool() || IsIconic(window).as_bool() {
                selection.visible.get_or_insert(window);
            } else {
                selection.hidden.get_or_insert(window);
            }
        }
    }
    true.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reused_process_identity_cannot_select_or_change_any_window() -> Result<(), DesktopError> {
        assert!(!activate_desktop_process(std::process::id(), 0)?);
        assert!(!activate_desktop_process(0, 0)?);
        Ok(())
    }
}
