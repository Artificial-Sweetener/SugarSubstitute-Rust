// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native-free owner policy tests never mint a verified peer or process-retirement proof.
use super::*;
use std::error::Error;
#[test]
fn root_recovery_close_preserves_first_exact_counter_and_local_cutoff_after_progress()
-> Result<(), Box<dyn Error>> {
    let mut close = None;
    let counter = CounterDeadline::new(900, 100)?;
    let until = Moment::from_nanos(15000);
    assert!(Closing::latch(&mut close, counter, until)?);
    let original = close.as_mut().ok_or("Missing original close")?;
    original.sent = true;
    original.control_closed = true;
    assert!(!Closing::latch(&mut close, counter, until)?);
    assert!(Closing::latch(&mut close, CounterDeadline::new(901, 100)?, until).is_err());
    assert!(Closing::latch(&mut close, counter, Moment::from_nanos(15001)).is_err());
    let original = close.as_ref().ok_or("Lost original close")?;
    assert_eq!(original.counter, counter);
    assert_eq!(original.until, until);
    assert!(original.sent && original.control_closed);
    Ok(())
}
#[test]
fn root_recovery_failure_owner_preserves_actual_first_and_secondary_sources_with_bounded_omissions()
-> Result<(), Box<dyn Error>> {
    let first = NativeObservationError::new(
        "first_recovery_native",
        std::io::Error::from_raw_os_error(109),
    );
    let second =
        NativeObservationError::new("second_recovery_native", std::io::Error::from_raw_os_error(6));
    let first_source = std::ptr::from_ref(first.source().ok_or("Missing first cause")?);
    let second_source = std::ptr::from_ref(second.source().ok_or("Missing second cause")?);
    let mut failures = failures::Failures::default();
    failures.record(SessionCause::Native(first));
    failures.record(SessionCause::Native(second));
    for _ in 0..100 {
        failures.record(SessionCause::Protocol(SessionCode::Order));
    }
    assert_eq!(failures.omitted, 100);
    for (stored, source) in
        [(failures.first.as_ref(), first_source), (failures.secondary.as_ref(), second_source)]
    {
        let actual = stored
            .ok_or("Lost failure")?
            .source()
            .and_then(Error::source)
            .ok_or("Lost actual native cause")?;
        assert!(std::ptr::eq(std::ptr::from_ref(actual), source));
    }
    failures.omitted = u64::MAX;
    failures.record(SessionCause::Protocol(SessionCode::Exhausted));
    assert_eq!(failures.omitted, u64::MAX);
    Ok(())
}
#[test]
fn root_recovery_metadata_is_bounded_without_root_path_copy_and_closed_owner_grants_no_proof()
-> Result<(), Box<dyn Error>> {
    assert!(RootRecoverySession::required_storage_bytes() <= 65536);
    assert_eq!(RootRecoverySession::worker_reservation_bytes(), 4096);
    let mut empty = RootRecoverySession { state: None, _outside: None };
    assert!(empty.take_retired().is_err());
    assert!(empty.failure().is_none());
    assert!(empty.with_desktop(|_| ()).is_err());
    assert!(empty.begin_close(CounterDeadline::new(1, 1)?, Moment::from_nanos(1)).is_err());
    let facts = empty.poll(Path::new(""), Moment::from_nanos(1));
    assert!(!facts.retired && !facts.handler_needed && !facts.desktop_needed);
    assert_eq!(
        crate::production::entry_activation::normal_idle::recovery_retirement(facts.retired),
        Err("RecoveryRetirement")
    );
    assert_eq!(crate::production::entry_activation::normal_idle::recovery_retirement(true), Ok(()));
    assert!(empty.take_retired().is_err());
    Ok(())
}
