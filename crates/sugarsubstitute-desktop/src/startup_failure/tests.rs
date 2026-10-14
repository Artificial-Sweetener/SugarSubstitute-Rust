// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify inert first-stage enrichment preserves original error ownership and closed formatting.
use super::*;

#[test]
fn startup_close_enrichment_keeps_first_stage_snapshot_and_original_typed_cause()
-> Result<(), Box<dyn Error>> {
    let error = std::io::Error::from(ErrorKind::TimedOut);
    let first = StartupCloseFailure::retain(
        StartupCloseStage::CaptureObservation,
        StartupCaptureState::ClockUnavailable,
        StartupCauseCode::TimedOut,
        Box::new(error),
    );
    let address =
        first.downcast_ref::<StartupCloseFailure>().ok_or("First envelope absent")?.original()
            as *const dyn Error;
    let second = StartupCloseFailure::retain(
        StartupCloseStage::PolicyApply,
        StartupCaptureState::Latched,
        StartupCauseCode::Opaque,
        first,
    );
    let actual = second.downcast_ref::<StartupCloseFailure>().ok_or("First envelope replaced")?;
    assert_eq!(actual.stage(), StartupCloseStage::CaptureObservation);
    assert_eq!(actual.capture(), StartupCaptureState::ClockUnavailable);
    assert_eq!(actual.code(), StartupCauseCode::TimedOut);
    assert!(std::ptr::addr_eq(address, actual.original() as *const dyn Error));
    assert_eq!(
        actual.original().downcast_ref::<std::io::Error>().ok_or("Original IO lost")?.kind(),
        ErrorKind::TimedOut
    );
    Ok(())
}

#[test]
fn startup_closed_codes_format_canonical_native_and_request_facts() {
    assert_eq!(
        StartupCauseCode::Native(StartupNativeCause::CaptureClock, None).to_string(),
        "Native(CaptureClock)"
    );
    assert_eq!(
        StartupCauseCode::Native(StartupNativeCause::CounterConversion, Some(u32::MAX)).to_string(),
        "NativeOs(CounterConversion,4294967295)"
    );
    assert_eq!(
        StartupCauseCode::Request(StartupRequestCause::Deadline).to_string(),
        "Request(Deadline)"
    );
    assert_eq!(StartupCauseCode::Os(i32::MIN).to_string(), "Os(-2147483648)");
    assert_eq!(
        StartupCauseCode::direct(&std::io::Error::from_raw_os_error(233)),
        StartupCauseCode::Os(233)
    );
    assert_eq!(
        StartupCauseCode::direct(&std::io::Error::from(ErrorKind::BrokenPipe)),
        StartupCauseCode::Disconnected
    );
}
