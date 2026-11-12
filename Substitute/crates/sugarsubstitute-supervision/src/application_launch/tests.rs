// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Actual windowless creation and checked native close failure survive discarded worker results.
use super::*;
mod containment;
mod image_errors;
use crate::setup_storage::test_worker::{self as worker, TestResult};
use windows::Win32::{
    Foundation::{
        HANDLE, HANDLE_FLAG_PROTECT_FROM_CLOSE, HANDLE_FLAGS, SetHandleInformation, WAIT_OBJECT_0,
    },
    System::Threading::{INFINITE, TerminateProcess, WaitForSingleObject},
};

#[test]
fn native_status_distinguishes_no_process_from_success_and_uncertain_outputs() {
    let empty = PROCESS_INFORMATION::default();
    assert_eq!(CreationOutcome::observed(false, &empty), CreationOutcome::NotCreated);
    assert_eq!(CreationOutcome::observed(true, &empty), CreationOutcome::Created);
    for process in [
        PROCESS_INFORMATION { hProcess: HANDLE(std::ptr::dangling_mut()), ..empty },
        PROCESS_INFORMATION { hThread: HANDLE(std::ptr::dangling_mut()), ..empty },
    ] {
        assert_eq!(CreationOutcome::observed(false, &process), CreationOutcome::Indeterminate);
    }
}

/// Observe this test's actual child through its owned handle, then retire any failed fixture.
pub(in crate::application_launch) fn settle_fixture(handle: HANDLE) -> Result<(), ClaimError> {
    // SAFETY: the caller retains the uniquely owned process handle under its state lock.
    unsafe { SetHandleInformation(handle, HANDLE_FLAG_PROTECT_FROM_CLOSE.0, HANDLE_FLAGS(0)) }
        .map_err(|cause| ClaimError::new("fixture_unprotect_process", cause))?;
    // SAFETY: the same actual child handle remains owned throughout the observation.
    if unsafe { WaitForSingleObject(handle, INFINITE) } == WAIT_OBJECT_0 {
        return Ok(());
    }
    // SAFETY: this is exclusively the windowless --list fixture created by this test, not a PID lookup.
    unsafe { TerminateProcess(handle, 1) }
        .map_err(|cause| ClaimError::new("fixture_terminate_process", cause))?;
    // SAFETY: termination is observed on the same retained kernel process object.
    if unsafe { WaitForSingleObject(handle, INFINITE) } != WAIT_OBJECT_0 {
        return Err(ClaimError::invalid("fixture_process_not_settled"));
    }
    Err(ClaimError::invalid("fixture_process_observation_expired"))
}

#[test]
fn actual_creation_survives_native_close_failure_and_expired_worker_completion() -> TestResult {
    let executable = std::env::current_exe()?;
    let command = WindowsLaunchCommand::from_os(
        &executable,
        executable.parent().ok_or("Fixture directory absent")?,
        &["--list".into(), "__sugar_application_launch_child_no_tests__".into()],
    )?;
    let launch = ApplicationLaunch::new(command);
    let owned = launch.clone();
    worker::run_reserved_expiring(1024 * 1024, move |context, clock| {
        let result = owned.launch_with(context, |command, custody, process| {
            creation::create(command, custody, process)?;
            // SAFETY: the just-created, uniquely owned process handle remains in the output slot.
            // Windows must reject its first CloseHandle until this test removes protection.
            unsafe {
                SetHandleInformation(
                    process.hProcess,
                    HANDLE_FLAG_PROTECT_FROM_CLOSE.0,
                    HANDLE_FLAG_PROTECT_FROM_CLOSE,
                )
            }
            .map_err(|cause| ClaimError::new("fixture_protect_process", cause))
        });
        let state = crate::native_claims::lock(&owned.0);
        let process = state
            .handles
            .iter()
            .find(|handle| handle.is_armed())
            .ok_or("Failed closure lost its process handle")?;
        settle_fixture(process.raw())?;
        assert_eq!(state.creation, CreationOutcome::Created);
        drop(state);
        assert_eq!(
            result.err().ok_or("Protected closure unexpectedly succeeded")?.operation(),
            "close_handle"
        );
        clock.expire();
        Ok(())
    })?;
    worker::run_reserved(1024 * 1024, move |context, _| {
        assert_eq!(launch.recover_and_retire(context)?, CreationOutcome::Created);
        assert_eq!(launch.recover_and_retire(context)?, CreationOutcome::Created);
        assert!(launch.launch(context).is_err());
        let state = crate::native_claims::lock(&launch.0);
        assert!(state.handles.is_empty());
        assert!(state.memory.is_none());
        Ok(())
    })
}
