// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own an offscreen qualification process and its desktop through actual process termination.
//!
//! Windows UIA/IME helpers can retain a desktop for their process lifetime. The parent therefore
//! owns the desktop object while one exact child test owns all native windows, COM and helper
//! threads. Process exit precedes desktop release; no test runs on the user's interactive desktop.
use crate::DesktopError;
use std::{
    os::windows::process::CommandExt,
    process::{Child, Command},
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0},
        System::{
            Com::CoCreateGuid,
            StationsAndDesktops::{
                CloseDesktop, CreateDesktopW, DESKTOP_CONTROL_FLAGS, DESKTOP_CREATEWINDOW,
                DESKTOP_READOBJECTS, DESKTOP_WRITEOBJECTS, HDESK, OpenDesktopW, SetThreadDesktop,
            },
            Threading::{
                CREATE_NO_WINDOW, CreateEventW, EVENT_MODIFY_STATE, OpenEventW, SetEvent,
                WaitForSingleObject,
            },
        },
    },
    core::PCWSTR,
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
const TARGET: &str = "SUGARSUBSTITUTE_NATIVE_TEST_TARGET";
const DESKTOP: &str = "SUGARSUBSTITUTE_NATIVE_TEST_DESKTOP";
const PREFIX: &str = "SugarSubstitute-Native-";
const ACCESS: u32 = DESKTOP_CREATEWINDOW.0 | DESKTOP_READOBJECTS.0 | DESKTOP_WRITEOBJECTS.0;

/// Reexecute this exact libtest case with its own process-scoped private desktop.
/// A completion event proves that the selected body actually ran; a successful empty test filter
/// cannot pass. Each caller remains parallel-safe and owns its independently supervised child.
/// The parent waits for the child's actual exit with no wall-clock budget: native work that is
/// slow on a loaded machine only makes the case slower, and a child that never exits shows as a
/// test that libtest reports as running long rather than as a load-dependent verdict.
#[cfg(test)]
pub(crate) fn run(name: &'static str, body: fn() -> TestResult) -> TestResult {
    run_for(env!("CARGO_CRATE_NAME"), name, body)
}
/// Restrict the cross-crate qualification port to the application's actual libtest namespace.
#[cfg(feature = "boundary-fixtures")]
pub(crate) fn host(name: &'static str, body: fn() -> TestResult) -> TestResult {
    run_for("sugarsubstitute", name, body)
}
/// Execute only the requested original test in the current executable on the owned desktop.
fn run_for(expected: &str, name: &'static str, body: fn() -> TestResult) -> TestResult {
    let (owner, target) = name.split_once("::").ok_or("native test path is incomplete")?;
    if owner != expected || target.is_empty() {
        return Err("native qualification test belongs to a different crate".into());
    }
    if let Some(selected) = std::env::var_os(TARGET) {
        if selected != target {
            return Err("native qualification target identity differs".into());
        }
        return run_child(body);
    }
    let mut owner = Case::new()?;
    let result = owner.execute(target);
    let closed = owner.close();
    match (result, closed) {
        (Err(cause), Err(closing)) => {
            Err(format!("{cause}; qualification retirement: {closing}").into())
        }
        (Err(cause), _) | (_, Err(cause)) => Err(cause),
        _ => Ok(()),
    }
}

/// Attach before any COM/windows exist. The owning parent retains the desktop until process exit.
fn run_child(body: fn() -> TestResult) -> TestResult {
    let name = std::env::var(DESKTOP)?;
    if !name.starts_with(PREFIX)
        || name.len() != PREFIX.len() + 38
        || name.contains(['\\', '/', '\0'])
    {
        return Err("invalid private qualification desktop identity".into());
    }
    let desktop_name = wide(&name);
    // SAFETY: Open only the parent's random private desktop, without desktop-switch rights.
    let desktop = unsafe {
        OpenDesktopW(PCWSTR(desktop_name.as_ptr()), DESKTOP_CONTROL_FLAGS(0), false, ACCESS)
    }?;
    // SAFETY: This fresh child test thread has not initialized COM or created any windows.
    if let Err(cause) = unsafe { SetThreadDesktop(desktop) } {
        // SAFETY: Failed attachment leaves the newly opened desktop handle unborrowed.
        unsafe { CloseDesktop(desktop) }?;
        return Err(cause.into());
    }
    // The child process owns this handle through its final native helper thread, rather than
    // attempting to close a desktop still used by Windows' process-scoped UIA/IME infrastructure.
    body()?;
    let event_name = wide(&format!("Local\\{name}"));
    // SAFETY: Open only the exact parent's completion event, with signal-only permission.
    let event = unsafe { OpenEventW(EVENT_MODIFY_STATE, false, PCWSTR(event_name.as_ptr())) }?;
    // SAFETY: The owned event remains valid through signal and checked close.
    let signaled = unsafe { SetEvent(event) };
    // SAFETY: This closes only the child's exact opened event reference.
    let closed = unsafe { CloseHandle(event) };
    signaled?;
    closed?;
    Ok(())
}

/// Parent-owned custody exists before any launch; failed partial construction retains cleanup.
struct Case {
    name: String,
    desktop: Option<HDESK>,
    completed: Option<HANDLE>,
    child: Option<Child>,
}
impl Case {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // SAFETY: The OS generates an independent name; COM initialization is not required.
        let guid = unsafe { CoCreateGuid() }?;
        let mut owner = Self {
            name: format!("{PREFIX}{{{guid:?}}}"),
            desktop: None,
            completed: None,
            child: None,
        };
        let name = wide(&owner.name);
        // SAFETY: The parent owns a newly created inactive desktop and never acquires switch rights.
        owner.desktop = Some(unsafe {
            CreateDesktopW(
                PCWSTR(name.as_ptr()),
                None,
                None,
                DESKTOP_CONTROL_FLAGS(0),
                ACCESS,
                None,
            )
        }?);
        let event = wide(&format!("Local\\{}", owner.name));
        // SAFETY: The parent owns this independently named manual-reset completion event.
        owner.completed = Some(unsafe { CreateEventW(None, true, false, PCWSTR(event.as_ptr())) }?);
        Ok(owner)
    }
    fn execute(&mut self, target: &str) -> TestResult {
        self.child = Some(
            Command::new(std::env::current_exe()?)
                .arg("--exact")
                .arg(target)
                .env(TARGET, target)
                .env(DESKTOP, &self.name)
                .creation_flags(CREATE_NO_WINDOW.0)
                .spawn()?,
        );
        let child = self.child.as_mut().ok_or("native child custody absent")?;
        let status = child.wait()?;
        if !status.success() {
            return Err(format!("native qualification child failed: {status}").into());
        }
        let event = self.completed.ok_or("native completion event absent")?;
        // SAFETY: Probe only this parent's retained completion event, without blocking.
        if unsafe { WaitForSingleObject(event, 0) } != WAIT_OBJECT_0 {
            return Err("native qualification child did not report its exact test body".into());
        }
        Ok(())
    }
    /// Stop only the exact spawned child, join its process, then release event and desktop handles.
    fn close(&mut self) -> TestResult {
        if let Some(child) = self.child.as_mut() {
            if child.try_wait()?.is_none() {
                child.kill()?;
            }
            // Termination is joined, not assumed: the desktop is released only after the exit.
            child.wait()?;
            self.child.take();
        }
        if let Some(event) = self.completed {
            // SAFETY: The child process has terminated before the parent's event is released.
            unsafe { CloseHandle(event) }?;
            self.completed = None;
        }
        if let Some(desktop) = self.desktop {
            // SAFETY: No thread in the terminated child can retain or use this parent's desktop.
            unsafe { CloseDesktop(desktop) }
                .map_err(|cause| DesktopError::new("test_desktop_close", cause))?;
            self.desktop = None;
        }
        Ok(())
    }
}
impl Drop for Case {
    fn drop(&mut self) {
        if let Err(cause) = self.close() {
            tracing::error!(operation="native_qualification_retirement",error=%cause,"Native qualification custody remains unsettled");
            if let Some(child) = self.child.take() {
                std::mem::forget(child);
            }
        }
    }
}
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}

#[test]
fn missing_child_case_cannot_pass_without_native_completion() -> TestResult {
    let mut owner = Case::new()?;
    let result = owner.execute("__absent_native_qualification_case__");
    let detail = result.err().ok_or("an empty child test filter was accepted")?.to_string();
    owner.close()?;
    assert!(detail.contains("did not report its exact test body"), "{detail}");
    assert!(owner.child.is_none() && owner.completed.is_none() && owner.desktop.is_none());
    Ok(())
}
