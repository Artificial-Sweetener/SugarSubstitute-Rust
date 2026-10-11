// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Actual event ABI fixtures through the production owner; root executes separately.
use super::*;
use crate::runtime_test_support::{Rig, TestResult, spec};
use sugarsubstitute_contracts::execution::*;
use windows::Win32::{
    Security::SECURITY_ATTRIBUTES,
    System::Threading::{CreateEventW, CreateMutexW},
};
fn create_inputs(wrong_object: bool) -> TestResult<InheritedFixtureEvents> {
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        bInheritHandle: true.into(),
        ..Default::default()
    };
    let mut handles = [0; 4];
    for index in 0..4 {
        // SAFETY: initialized attributes; no names or borrowed security descriptors.
        let result = unsafe {
            if wrong_object && index == 0 {
                CreateMutexW(Some(&attributes), false, None)
            } else {
                CreateEventW(Some(&attributes), true, false, None)
            }
        };
        match result {
            Ok(handle) => handles[index] = handle.0 as usize,
            Err(cause) => {
                let mut error = NativeObservationError::new("ffi_fixture_create", cause);
                for raw in &handles[..index] {
                    // SAFETY: unique fixture output not yet transferred; preserve failed closure.
                    if let Err(cause) = unsafe { CloseHandle(HANDLE(*raw as *mut _)) } {
                        error.retain_cleanup(NativeObservationError::new(
                            "ffi_fixture_close",
                            cause,
                        ));
                    }
                }
                return Err(Box::new(error));
            }
        }
    }
    Ok(InheritedFixtureEvents::new(
        handles[0] as u64,
        handles[1] as u64,
        handles[2] as u64,
        handles[3] as u64,
    )?)
}
fn reserve(rig: &Rig) -> TestResult<(Scope, FixtureForwardingPermit)> {
    let scope = rig.client.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(2)?, deadline: None },
        None,
        true,
    )?;
    let mut specification =
        spec(1, JobClass::NativeObservation, crate::fixture_session::MIN_FIXTURE_SESSION_BYTES)?;
    specification.operation = Operation::NativeLifecycle;
    let permit = rig.client.reserve_fixture_forwarding(
        &scope,
        specification,
        crate::windows::NativeCorrelation::new(1)?,
        crate::windows_fixture::FixtureDeadline::until(
            std::time::Instant::now()
                .checked_add(std::time::Duration::from_secs(10))
                .ok_or("deadline")?,
        )?,
    )?;
    Ok((scope, permit))
}
#[test]
fn ffi_actual_four_events_prepare_and_checked_close_on_native_worker() -> TestResult {
    let rig = Rig::new()?;
    let (_scope, permit) = reserve(&rig)?;
    let mut session = permit.bind(create_inputs(false)?);
    loop {
        let before = rig.wake.count();
        let _ = rig.client.drain_completions(std::num::NonZeroUsize::new(32).ok_or("budget")?)?;
        if let Some(pin) = session.control.take_events()? {
            drop(pin);
            break;
        }
        rig.wake.wait_after(before)?;
    }
    assert!(
        matches!(rig.completion(&mut session.completion)?.outcome,JobOutcome::Success(report) if report.prepared)
    );
    Ok(())
}
#[test]
fn ffi_actual_mutex_input_fails_event_validation_and_closes_before_terminal() -> TestResult {
    let rig = Rig::new()?;
    let (_scope, permit) = reserve(&rig)?;
    let mut session = permit.bind(create_inputs(true)?);
    assert!(
        matches!(rig.completion(&mut session.completion)?.outcome,JobOutcome::Failure(ref cause)
        if cause.stage()=="forwarding_validate_event")
    );
    Ok(())
}
