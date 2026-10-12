// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Retain actual registered callback/output ownership until checked unregister succeeds.
use super::*;
use crate::{
    native_pipe::handles::{Handle, adopt},
    runtime_test_support::TestResult,
    symbol_test_support::LiveSymbol,
};
use windows::Win32::{Foundation::ERROR_GEN_FAILURE, System::Threading::CreateEventW};

#[test]
fn symbol_failed_unregister_retains_callback_and_credit_until_actual_quiescence() -> TestResult {
    let live = LiveSymbol::new()?;
    let reservation = live.transport()?;
    let effect = crate::native_resources::EffectContext::new(&reservation);
    let mut event = Handle::new();
    event.attach(reservation.handles.clone(), effect.clone());
    event.reserve("fixture_wait_event")?;
    // SAFETY: initially unsignalled private event is adopted immediately by its actual owner.
    let raw = unsafe { CreateEventW(None, true, false, None) }?;
    adopt(&mut event, raw, "fixture_wait_event")?;
    let mut wait = Wait::new(Signal::new(&live.rig.client.core));
    wait.attach(reservation.handles.clone(), effect);
    wait.register(event.raw())?;
    let callback = wait.owner.as_ref().map(|owner| owner.callback.as_ref() as *const Callback);
    assert!(wait.retained());
    assert_eq!(reservation.handles.in_custody(), 2);
    let failure = wait
        .retire_with(|_| {
            Err(windows::core::Error::from_hresult(windows::core::HRESULT::from_win32(
                ERROR_GEN_FAILURE.0,
            )))
        })
        .err()
        .ok_or("Failed unregister unexpectedly settled")?;
    assert_eq!(failure.stage(), "unregister_process_wait");
    assert!(wait.retained());
    assert_eq!(
        wait.owner.as_ref().map(|owner| owner.callback.as_ref() as *const Callback),
        callback
    );
    assert_eq!(reservation.handles.in_custody(), 2);
    live.rig.scope.cancel();
    wait.retire()?;
    assert!(!wait.retained());
    assert_eq!(reservation.handles.in_custody(), 1);
    event.close("fixture_retire_wait_event")?;
    assert_eq!(reservation.handles.in_custody(), 0);
    Ok(())
}
