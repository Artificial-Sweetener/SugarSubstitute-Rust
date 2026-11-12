// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bring forward, probe or end an authenticated SugarSubstitute process by its verified identity.
//!
//! Every operation re-opens the process and compares its creation time with the identity the
//! caller authenticated, so a reused process id never reaches another program. Window queries
//! never wait on the target's UI thread except through a bounded responsiveness probe.
use crate::DesktopError;
use std::{
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    time::Duration,
};
use windows::Win32::{Foundation::*, System::Threading::*, UI::WindowsAndMessaging::*};

/// How long one window may take to answer the responsiveness probe.
const PROBE_REPLY: Duration = Duration::from_millis(1000);
/// Top-level windows probed per process; a process's windows share few UI threads.
const MAX_PROBED_WINDOWS: usize = 16;

/// A failed native operation on a verified process, with its stage and original cause.
/// Unlike window-thread failures it crosses worker threads, so it carries only sendable data.
#[derive(Debug)]
pub struct ProcessError {
    operation: &'static str,
    cause: windows::core::Error,
}
impl ProcessError {
    fn new(operation: &'static str, cause: windows::core::Error) -> Self {
        Self { operation, cause }
    }
    /// The stage that failed.
    pub fn operation(&self) -> &'static str {
        self.operation
    }
}
impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Process operation {} failed", self.operation)
    }
}
impl std::error::Error for ProcessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}

/// Whether an authenticated process's windows answer their message queues.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessResponsiveness {
    /// Every probed window answered, or the process has no window to probe yet.
    Responsive,
    /// A window did not answer within the probe bound, or Windows already reports it hung.
    Unresponsive,
    /// The process has exited, or the id now names another process.
    Exited,
}

/// Restore a running desktop after a repeated launch, without waiting on its UI thread.
/// The caller supplies the PID and creation time from its authenticated desktop channel.
/// Returns false after process exit or before a top-level application window exists.
/// Foreground selection remains subject to Windows focus policy; a denied focus request
/// still restores the visible window. Native query failures preserve their cause.
pub fn activate_desktop_process(pid: u32, creation_time_100ns: u64) -> Result<bool, DesktopError> {
    let verified = open_verified(pid, creation_time_100ns, PROCESS_SYNCHRONIZE)
        .map_err(|cause| DesktopError::new("activate_desktop_process", cause))?;
    if verified.is_none() {
        return Ok(false);
    }
    restore_window(pid)
}

/// Ask each top-level window of an authenticated process to answer a null message, waiting at
/// most one second per window. This blocks the calling thread, so it runs in an owned
/// blocking job, never on a UI thread. A window Windows already reports hung answers at once.
/// # Errors
/// Native query failures other than process exit or window disappearance preserve their cause.
pub fn probe_process_responsiveness(
    pid: u32,
    creation_time_100ns: u64,
) -> Result<ProcessResponsiveness, ProcessError> {
    if open_verified(pid, creation_time_100ns, PROCESS_SYNCHRONIZE)?.is_none() {
        return Ok(ProcessResponsiveness::Exited);
    }
    let mut windows = Windows { pid, found: [None; MAX_PROBED_WINDOWS], count: 0 };
    // SAFETY: EnumWindows invokes the callback synchronously with this live stack value.
    unsafe { EnumWindows(Some(collect), LPARAM((&raw mut windows) as isize)) }
        .map_err(|cause| ProcessError::new("probe_process_windows", cause))?;
    let timeout = u32::try_from(PROBE_REPLY.as_millis()).unwrap_or(u32::MAX);
    for window in windows.found.into_iter().flatten() {
        // SAFETY: WM_NULL carries no pointers. SMTO_ABORTIFHUNG returns at once for a window
        // Windows already reports hung; otherwise the call waits at most `timeout`.
        let answered = unsafe {
            SendMessageTimeoutW(
                window,
                WM_NULL,
                WPARAM(0),
                LPARAM(0),
                SMTO_ABORTIFHUNG | SMTO_BLOCK,
                timeout,
                None,
            )
        };
        if answered.0 != 0 {
            continue;
        }
        let cause = windows::core::Error::from_thread();
        if cause.code() == windows::core::HRESULT::from_win32(ERROR_TIMEOUT.0) {
            return Ok(ProcessResponsiveness::Unresponsive);
        }
        // SAFETY: a transient HWND query; a destroyed window is a benign result.
        if !unsafe { IsWindow(Some(window)) }.as_bool() {
            continue;
        }
        return Err(ProcessError::new("probe_process_window", cause));
    }
    if open_verified(pid, creation_time_100ns, PROCESS_SYNCHRONIZE)?.is_none() {
        return Ok(ProcessResponsiveness::Exited);
    }
    Ok(ProcessResponsiveness::Responsive)
}

/// End an authenticated process outright after the user chose to close an unresponsive copy.
/// Returns false when it had already exited. Its kill-on-close job ends its children with it.
/// # Errors
/// Access or termination failures preserve their native cause.
pub fn end_process(pid: u32, creation_time_100ns: u64) -> Result<bool, ProcessError> {
    let Some(process) =
        open_verified(pid, creation_time_100ns, PROCESS_TERMINATE | PROCESS_SYNCHRONIZE)?
    else {
        return Ok(false);
    };
    // SAFETY: the owned handle names the verified process and carries terminate rights.
    unsafe { TerminateProcess(HANDLE(process.as_raw_handle()), 1) }
        .map_err(|cause| ProcessError::new("end_process", cause))?;
    Ok(true)
}

/// Wait for an authenticated process to exit, at most `limit`. Returns true once it has exited
/// or its id names another process. Blocks the calling thread on the process handle, so it runs
/// in an owned blocking job.
/// # Errors
/// Native wait failures preserve their cause.
pub fn wait_for_process_exit(
    pid: u32,
    creation_time_100ns: u64,
    limit: Duration,
) -> Result<bool, ProcessError> {
    let Some(process) = open_verified(pid, creation_time_100ns, PROCESS_SYNCHRONIZE)? else {
        return Ok(true);
    };
    let milliseconds = u32::try_from(limit.as_millis()).unwrap_or(INFINITE - 1);
    // SAFETY: the owned process handle lives through this bounded wait.
    match unsafe { WaitForSingleObject(HANDLE(process.as_raw_handle()), milliseconds) } {
        WAIT_OBJECT_0 => Ok(true),
        WAIT_TIMEOUT => Ok(false),
        _ => Err(ProcessError::new("wait_for_process_exit", windows::core::Error::from_thread())),
    }
}

/// Open `pid` with `access` plus query rights when it is still the process born at `birth`.
/// Returns `None` when it has exited or the id has been reused.
fn open_verified(
    pid: u32,
    birth: u64,
    access: PROCESS_ACCESS_RIGHTS,
) -> Result<Option<OwnedHandle>, ProcessError> {
    // SAFETY: the returned handle is adopted immediately and owned through identity checks.
    let raw = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | access, false, pid) }
    {
        Ok(raw) => raw,
        Err(cause)
            if cause.code() == windows::core::HRESULT::from_win32(ERROR_INVALID_PARAMETER.0) =>
        {
            return Ok(None);
        }
        Err(cause) => return Err(ProcessError::new("open_verified_process", cause)),
    };
    // SAFETY: OpenProcess returned this uniquely owned kernel handle.
    let process = unsafe { OwnedHandle::from_raw_handle(raw.0) };
    let handle = HANDLE(process.as_raw_handle());
    let (mut created, mut exited, mut kernel, mut user) =
        (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
    // SAFETY: the owned process and four writable FILETIME values live through the call.
    unsafe { GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) }
        .map_err(|cause| ProcessError::new("verified_process_identity", cause))?;
    let created = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
    // SAFETY: a zero-duration query never blocks the caller or target thread.
    if created != birth || unsafe { WaitForSingleObject(handle, 0) } != WAIT_TIMEOUT {
        return Ok(None);
    }
    Ok(Some(process))
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
    let mut title = [0u16; 2];
    // SAFETY: these queries accept transient HWNDs and retain no references.
    // InternalGetWindowText reads the stored title without sending a message, so a hung
    // window cannot block the enumeration.
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut pid));
        if pid == selection.pid
            && GetAncestor(window, GA_ROOTOWNER) == window
            && GetWindowLongPtrW(window, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 == 0
            && InternalGetWindowText(window, &mut title) > 0
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

struct Windows {
    pid: u32,
    found: [Option<HWND>; MAX_PROBED_WINDOWS],
    count: usize,
}

/// Collect every top-level window the process owns, hidden helper windows included: any of
/// them failing to answer means its UI thread is not pumping messages.
unsafe extern "system" fn collect(window: HWND, context: LPARAM) -> windows::core::BOOL {
    // SAFETY: the only caller passes its live Windows value for this synchronous enumeration.
    let windows = unsafe { &mut *(context.0 as *mut Windows) };
    let mut pid = 0;
    // SAFETY: a transient HWND query that retains no reference.
    unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
    if pid == windows.pid && windows.count < MAX_PROBED_WINDOWS {
        windows.found[windows.count] = Some(window);
        windows.count += 1;
    }
    // Stopping early would make EnumWindows report a failure with no cause.
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
    #[test]
    fn a_reused_identity_is_never_probed_ended_or_awaited() -> Result<(), ProcessError> {
        let pid = std::process::id();
        assert_eq!(probe_process_responsiveness(pid, 0)?, ProcessResponsiveness::Exited);
        assert!(!end_process(pid, 0)?);
        assert!(wait_for_process_exit(pid, 0, Duration::ZERO)?);
        Ok(())
    }
}
