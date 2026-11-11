// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Exercise checked actual handles against original admission and retained output custody.
use super::*;
use crate::{runtime_test_support::TestResult, symbol_test_support::LiveSymbol};
use windows::Win32::System::Threading::CreateEventW;
fn event(live: &LiveSymbol, reservation: &crate::SymbolTransportReservation) -> TestResult<Handle> {
    let mut handle = Handle::new();
    handle.attach(crate::native_pipe::accounting::OutputAccounting::symbol(
        reservation.handles.clone(),
        crate::native_resources::EffectContext::new(reservation),
    ));
    handle.reserve("symbol_event_create")?;
    // SAFETY: a unique private initially unsignalled event is immediately adopted by its durable slot.
    let raw = unsafe { CreateEventW(None, true, false, None) }?;
    adopt(&mut handle, raw, "symbol_event_create")?;
    assert_eq!(live.rig.scope.snapshot().native_outputs_in_custody, 1);
    Ok(handle)
}
#[test]
fn symbol_stopped_original_blocks_fresh_create_while_checked_retirement_releases_actual_output()
-> TestResult {
    let live = LiveSymbol::new()?;
    let reservation = live.transport()?;
    let mut handle = event(&live, &reservation)?;
    live.rig.clock.advance(10_000_000_000);
    assert!(handle.close_normal("symbol_close_normal").is_err());
    assert_ne!(handle.value(), 0);
    assert_eq!(reservation.handles.in_custody(), 1);
    let mut refused = Handle::new();
    refused.attach(crate::native_pipe::accounting::OutputAccounting::symbol(
        reservation.handles.clone(),
        crate::native_resources::EffectContext::new(&reservation),
    ));
    assert!(refused.reserve("symbol_new_event_after_expiry").is_err());
    assert_eq!(reservation.handles.in_custody(), 1);
    handle.close("symbol_retire_event")?;
    assert_eq!(handle.value(), 0);
    assert_eq!(reservation.handles.in_custody(), 0);
    Ok(())
}
#[test]
fn symbol_failed_actual_close_retains_handle_guard_and_first_cause_until_checked_retry()
-> TestResult {
    let live = LiveSymbol::new()?;
    let reservation = live.transport()?;
    let mut handle = event(&live, &reservation)?;
    // SAFETY: this fixture exclusively owns the actual event and deliberately protects only its close.
    unsafe {
        SetHandleInformation(
            handle.raw(),
            HANDLE_FLAG_PROTECT_FROM_CLOSE.0,
            HANDLE_FLAG_PROTECT_FROM_CLOSE,
        )
    }?;
    let first = handle
        .close("symbol_protected_close")
        .err()
        .ok_or("Protected handle unexpectedly closed")?;
    assert_eq!(first.cause.stage(), "symbol_protected_close");
    assert_ne!(handle.value(), 0);
    assert_eq!(reservation.handles.in_custody(), 1);
    assert_eq!(
        handle
            .owner
            .as_ref()
            .and_then(|owner| owner.cause.as_ref())
            .map(NativeObservationError::stage),
        Some("symbol_protected_close")
    );
    // SAFETY: clear this fixture's protection on the same still-owned actual event.
    unsafe {
        SetHandleInformation(handle.raw(), HANDLE_FLAG_PROTECT_FROM_CLOSE.0, HANDLE_FLAGS(0))
    }?;
    handle.close("symbol_checked_close")?;
    assert_eq!(reservation.handles.in_custody(), 0);
    Ok(())
}
#[test]
fn symbol_actual_core_cancel_during_pre_effect_clock_returns_without_creation_or_output_credit()
-> TestResult {
    let live = LiveSymbol::new()?;
    let reservation = live.transport()?;
    let mut handle = Handle::new();
    handle.attach(crate::native_pipe::accounting::OutputAccounting::symbol(
        reservation.handles.clone(),
        crate::native_resources::EffectContext::new(&reservation),
    ));
    let client = live.rig.client.clone();
    let job_id = live.context.job_id();
    live.rig.clock.on_next_current_thread_observation(Box::new(move || {
        client.core_for_test().cancel_job(job_id)
    }));
    assert!(handle.reserve("symbol_reentrant_create").is_err());
    assert_eq!(handle.value(), 0);
    assert_eq!(reservation.handles.in_custody(), 0);
    Ok(())
}

#[test]
fn symbol_partial_output_is_adopted_before_failed_abi_status_and_retired_explicitly() -> TestResult
{
    let live = LiveSymbol::new()?;
    let reservation = live.transport()?;
    let mut slot = Handle::new();
    slot.attach(crate::native_pipe::accounting::OutputAccounting::symbol(
        reservation.handles.clone(),
        crate::native_resources::EffectContext::new(&reservation),
    ));
    slot.reserve("fixture_partial_output")?;
    // SAFETY: external-boundary fixture returns a real private output with failed ABI status.
    let raw = unsafe { CreateEventW(None, true, false, None) }?;
    let status = Err(windows::core::Error::from_hresult(windows::core::HRESULT::from_win32(
        ERROR_GEN_FAILURE.0,
    )));
    let cause = slot
        .finish_output(raw, status, "fixture_partial_output")
        .err()
        .ok_or("Partial failure accepted")?;
    assert_eq!(cause.cause.stage(), "fixture_partial_output");
    assert_eq!(slot.raw(), raw);
    assert_eq!(reservation.handles.in_custody(), 1);
    live.rig.scope.cancel();
    slot.close("fixture_partial_retirement")?;
    assert_eq!(slot.value(), 0);
    assert_eq!(reservation.handles.in_custody(), 0);
    Ok(())
}

struct DiagnosticPanic;
impl tracing::Subscriber for DiagnosticPanic {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, _: &tracing::Event<'_>) {
        std::panic::resume_unwind(Box::new("native diagnostic subscriber".to_owned()));
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}
#[test]
fn symbol_diagnostic_panic_cannot_reclaim_actual_uncertain_handle_or_original_credit() -> TestResult
{
    let live = LiveSymbol::new()?;
    let reservation = live.transport()?;
    let handle = event(&live, &reservation)?;
    let owner = handle.owner.as_ref().ok_or("Actual physical owner absent")?.as_ref()
        as *const super::Owner;
    let diagnostic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        tracing::subscriber::with_default(DiagnosticPanic, || drop(handle));
    }));
    assert!(diagnostic.is_err());
    assert_eq!(reservation.handles.in_custody(), 1);
    assert_eq!(live.rig.scope.snapshot().reserved_bytes, crate::symbol_admission::MAIN_BYTES);
    // SAFETY: this fixture recorded its exclusive boxed owner before Drop; production
    // irrevocably leaked that same allocation before the subscriber panicked. All
    // diagnostic borrows have unwound. Reconstitute only this private fixture box
    // for checked physical retirement, without any production recovery port.
    let owner = unsafe { Box::from_raw(owner.cast_mut()) };
    let mut retained = Handle { owner: Some(owner) };
    assert_ne!(retained.value(), 0);
    live.rig.scope.cancel();
    retained.close("fixture_diagnostic_retirement")?;
    assert_eq!(reservation.handles.in_custody(), 0);
    Ok(())
}
