// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Compiled native ABI fixtures; root schedules their actual Windows qualification separately.

use super::*;
use crate::runtime_test_support::{Rig, TestResult, spec};
use ::windows::Win32::{
    Foundation::{DUPLICATE_SAME_ACCESS, DuplicateHandle, HANDLE},
    System::Threading::{CreateEventW, GetCurrentProcess, PROCESS_SYNCHRONIZE},
};
use std::error::Error;
use std::os::windows::io::FromRawHandle;
use sugarsubstitute_contracts::execution::*;

fn request(rig: &Rig, scope: &Scope) -> TestResult<ProcessWatchPermit> {
    let mut request = spec(1, JobClass::NativeObservation, 4096)?;
    request.operation = Operation::NativeLifecycle;
    Ok(rig.client.reserve_process_watch(scope, request, NativeCorrelation::new(1)?)?)
}
fn native_scope(rig: &Rig) -> TestResult<Scope> {
    Ok(rig.client.create_native_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?)
}
fn process_input(access: u32, same_access: bool) -> TestResult<OwnedHandle> {
    let mut owned = HANDLE::default();
    let options = if same_access { DUPLICATE_SAME_ACCESS } else { Default::default() };
    // SAFETY: current-process pseudo handles are borrowed; success returns one
    // unique noninheritable handle whose ownership immediately enters OwnedHandle.
    unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            GetCurrentProcess(),
            GetCurrentProcess(),
            &mut owned,
            access,
            false,
            options,
        )
    }?;
    // SAFETY: successful DuplicateHandle provided this unique valid owned handle.
    Ok(unsafe { OwnedHandle::from_raw_handle(owned.0) })
}

#[test]
fn native_unbound_cancellation_delivers_retirement_without_a_process_handle() -> TestResult {
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let mut job = request(&rig, &scope)?.cancel_unbound();
    let completion = rig.completion(&mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::Cancelled));
    drop(completion.memory);
    drop(rig.owner);
    assert_eq!(scope.snapshot().terminal_unobserved, 0);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn native_api_rejects_ordinary_class_before_accepting_a_process_handle() -> TestResult {
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    assert!(matches!(
        rig.client.reserve_process_watch(
            &scope,
            spec(1, JobClass::IoLifecycle, 4096)?,
            NativeCorrelation::new(1)?
        ),
        Err(AdmissionError::WrongClass { .. })
    ));
    Ok(())
}

#[test]
fn native_wrong_object_returns_original_process_identity_error() -> TestResult {
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let permit = request(&rig, &scope)?;
    // SAFETY: unnamed event has no borrowed security/name storage; success is
    // transferred immediately into unique Rust handle ownership, then the port.
    let event = unsafe { CreateEventW(None, false, false, ::windows::core::PCWSTR::null()) }?;
    // SAFETY: event is the unique valid owned handle returned above.
    let mut job = permit.bind(unsafe { OwnedHandle::from_raw_handle(event.0) });
    let completion = rig.completion(&mut job)?;
    let JobOutcome::Failure(error) = completion.outcome else {
        return Err("Wrong object unexpectedly accepted".into());
    };
    assert_eq!(error.stage(), "process_id");
    assert!(error.source().is_some());
    drop(completion.memory);
    drop(rig.owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn native_insufficient_query_rights_remain_typed_and_owned() -> TestResult {
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let permit = request(&rig, &scope)?;
    let mut job = permit.bind(process_input(PROCESS_SYNCHRONIZE.0, false)?);
    let completion = rig.completion(&mut job)?;
    let JobOutcome::Failure(error) = completion.outcome else {
        return Err("Insufficient process rights unexpectedly accepted".into());
    };
    assert_eq!(error.stage(), "process_id");
    assert!(error.source().is_some());
    Ok(())
}

#[test]
fn native_current_process_cancellation_never_terminates_authoritative_process() -> TestResult {
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let mut job = request(&rig, &scope)?.bind(process_input(0, true)?);
    job.cancel();
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::Cancelled));
    // Reaching the caller after cleanup establishes that observation cancellation
    // did not terminate the authoritative current process.
    drop(rig.owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
