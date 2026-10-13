// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual input, evidence and cleanup decisions without native effects.
use super::*;
use std::{ffi::OsString, time::Duration};
use sugarsubstitute_contracts::{
    crash::CaptureHandlerIdentity,
    lifecycle::{CoordinatorEpoch, NamespaceId},
};
#[test]
fn native_failure_diagnostic_preserves_only_the_numeric_os_code() {
    let cause = std::io::Error::from_raw_os_error(5);
    assert_eq!(receipt::native_os_code(&cause), Some(5));
    let private = std::io::Error::other("private credential/path text");
    assert_eq!(receipt::native_os_code(&private), None);
}
fn key() -> Result<CoordinatorKey> {
    Ok(CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    })
}
fn arguments() -> Vec<OsString> {
    [
        "--capture-root",
        r"D:\proof\protected",
        "--namespace",
        &"03".repeat(32),
        "--epoch",
        &"04".repeat(16),
        "--receipt",
        r"D:\proof\receipt.json",
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
}
#[test]
fn exact_role_inputs_preserve_keys_root_and_four_mebibyte_request() -> Result {
    let options = Options::parse(arguments())?;
    assert_eq!(options.coordinator.namespace.bytes(), &[3; 32]);
    assert_eq!(options.coordinator.epoch.bytes(), &[4; 16]);
    assert_eq!(
        options.input.as_ref().ok_or("Missing original input")?.required_reservation_bytes(),
        4_194_304
    );
    assert!(options.input.is_some());
    Ok(())
}
#[test]
fn role_rejects_unknown_reordered_trailing_uppercase_and_foreign_receipt_drive() {
    for (index, value) in [
        (0, "--other"),
        (2, "--epoch"),
        (3, "GG"),
        (5, "00000000000000000000000000000000"),
        (7, r"C:\receipt.json"),
    ] {
        let mut args = arguments();
        args[index] = value.into();
        assert!(Options::parse(args).is_err());
    }
    let mut args = arguments();
    args.push("trailing".into());
    assert!(Options::parse(args).is_err());
    let mut args = arguments();
    args[3] = "AB".repeat(32).into();
    assert!(Options::parse(args).is_err());
}
#[test]
fn actual_evidence_sequence_requires_matched_registration_exit_and_retirement() -> Result {
    let owner = key()?;
    let handler = CaptureHandlerIdentity { process_id: 17, creation_time_100ns: 23 };
    let mut evidence = Evidence::new(owner);
    evidence.observe(CaptureObservation::Registered { owner, handler })?;
    assert!(!evidence.complete());
    evidence.observe(CaptureObservation::HandlerExited { owner, handler, exit_code: 1 })?;
    assert!(!evidence.complete());
    evidence.observe(CaptureObservation::Closed { owner })?;
    assert!(evidence.complete());
    assert!(evidence.observe(CaptureObservation::Closed { owner }).is_err());
    Ok(())
}
#[test]
fn foreign_replayed_or_mismatching_facts_cannot_overwrite_existing_evidence() -> Result {
    let owner = key()?;
    let handler = CaptureHandlerIdentity { process_id: 17, creation_time_100ns: 23 };
    let mut evidence = Evidence::new(owner);
    evidence.observe(CaptureObservation::Registered { owner, handler })?;
    let foreign = CoordinatorKey { epoch: CoordinatorEpoch::new([3; 16])?, ..owner };
    assert!(evidence.observe(CaptureObservation::Closed { owner: foreign }).is_err());
    assert!(evidence.observe(CaptureObservation::Registered { owner, handler }).is_err());
    assert!(
        evidence
            .observe(CaptureObservation::HandlerExited {
                owner,
                handler: CaptureHandlerIdentity { creation_time_100ns: 24, ..handler },
                exit_code: 1
            })
            .is_err()
    );
    assert_eq!(evidence.handler, Some(handler));
    assert_eq!(evidence.exit, None);
    assert!(!evidence.closed);
    Ok(())
}
#[test]
fn failed_or_duplicate_close_terminals_do_not_rearm_without_new_cleanup_demand() -> Result {
    let mut demand = CleanupDemand::new();
    demand.request();
    assert!(demand.take());
    let id = demand.next_job()?;
    demand.admitted(id);
    demand.terminal(id, TerminalKind::Failure);
    for _ in 0..8 {
        assert!(!demand.take());
    }
    demand.terminal(id, TerminalKind::Success);
    assert!(!demand.take());
    demand.request();
    assert!(demand.take());
    let next = demand.next_job()?;
    assert_eq!(next, id + 1);
    demand.admitted(next);
    demand.terminal(next, TerminalKind::Success);
    assert!(demand.take());
    assert!(!demand.take());
    assert_eq!(demand.successful_terminals, 1);
    assert_eq!(demand.failed_terminals, 1);
    assert!(!Evidence::new(key()?).complete());
    Ok(())
}
#[test]
fn close_admission_is_bounded_and_registration_identity_always_precedes_close() -> Result {
    let mut demand = CleanupDemand::new();
    for expected in 3..259 {
        let id = demand.next_job()?;
        assert_eq!(id, expected);
        assert!(specification::START_JOB < id);
        demand.admitted(id);
        demand.terminal(id, TerminalKind::Success);
    }
    assert!(demand.next_job().is_err());
    assert_eq!(demand.admitted, 256);
    Ok(())
}
#[test]
fn capacity_wakes_cannot_acknowledge_private_capture_work_before_its_terminal_notice() -> Result {
    let mut demand = CleanupDemand::new();
    assert!(demand.can_observe_capture(false, false));
    assert!(!demand.can_observe_capture(true, false));
    assert!(demand.can_observe_capture(true, true));
    let close = demand.next_job()?;
    demand.admitted(close);
    for _ in 0..8 {
        assert!(!demand.can_observe_capture(true, true));
        demand.terminal(specification::WATCH_JOB, TerminalKind::Success);
    }
    assert!(!demand.can_observe_capture(true, true));
    demand.terminal(close, TerminalKind::Success);
    assert!(demand.can_observe_capture(true, true));
    assert!(demand.take());
    let next = demand.next_job()?;
    demand.admitted(next);
    demand.terminal(close, TerminalKind::Success);
    assert!(!demand.can_observe_capture(true, true));
    demand.terminal(next, TerminalKind::Failure);
    assert!(demand.can_observe_capture(true, true));
    assert!(!demand.take());
    Ok(())
}
#[test]
fn original_alarm_conversion_and_shutdown_boundaries_never_reset_or_spin() -> Result {
    let native = Instant::now();
    let start = Moment::from_nanos(17);
    let mut stop = StopPlan::new(start, Moment::from_nanos(19), native, Moment::from_nanos(23))?;
    assert_eq!(stop.budget.graceful_until.as_nanos(), 10_000_000_017);
    assert_eq!(stop.budget.settle_until.as_nanos(), 15_000_000_017);
    assert_eq!(stop.next_deadline(), native.checked_add(Duration::from_nanos(9_999_999_994)));
    assert_eq!(stop.due(stop.budget.graceful_until), (true, false));
    assert_eq!(stop.due(stop.budget.graceful_until), (false, false));
    assert_eq!(stop.due(stop.budget.settle_until), (false, true));
    assert_eq!(stop.due(stop.budget.settle_until), (false, false));
    assert_eq!(stop.next_deadline(), None);
    assert!(
        state::map_deadline(start, Moment::from_nanos(19), native, Moment::from_nanos(18)).is_err()
    );
    Ok(())
}
#[test]
fn post_teardown_writer_failure_preserves_first_cause_and_redacts_both() -> Result {
    let result = FixtureResult {
        path: r"D:\proof\receipt.json".into(),
        receipt: serde_json::json!({"bounded":true}),
        outcome: Err(std::io::Error::other("private-primary").into()),
    };
    let error = result
        .finish_with(|_, bytes| {
            assert!(bytes.len() <= MAX_RECEIPT_BYTES);
            Err(std::io::Error::other("private-writer").into())
        })
        .err()
        .ok_or("Missing grouped cause")?;
    assert_eq!(error.source().ok_or("Missing first source")?.to_string(), "private-primary");
    assert!(!format!("{error:?} {error}").contains("private"));
    Ok(())
}

#[test]
fn partial_handler_exit_and_native_closure_do_not_certify_registration() -> Result {
    let owner = key()?;
    let handler = CaptureHandlerIdentity { process_id: 17, creation_time_100ns: 23 };
    let mut evidence = Evidence::new(owner);
    evidence.observe(CaptureObservation::HandlerExited { owner, handler, exit_code: 1 })?;
    assert!(evidence.observe(CaptureObservation::Registered { owner, handler }).is_err());
    evidence.observe(CaptureObservation::Closed { owner })?;
    assert_eq!(evidence.handler, Some(handler));
    assert_eq!(evidence.exit, Some(1));
    assert!(evidence.closed);
    assert!(!evidence.complete());
    Ok(())
}

#[test]
fn oversized_receipt_preserves_primary_failure_before_any_writer_effect() -> Result {
    let result = FixtureResult {
        path: r"D:\proof\receipt.json".into(),
        receipt: serde_json::json!({ "oversized": "x".repeat(MAX_RECEIPT_BYTES) }),
        outcome: Err(std::io::Error::other("original-registration-cause").into()),
    };
    let mut writes = 0;
    let error = result
        .finish_with(|_, _| {
            writes += 1;
            Ok(())
        })
        .err()
        .ok_or("Missing bounded receipt failure")?;
    assert_eq!(writes, 0);
    assert_eq!(
        error.source().ok_or("Missing primary cause")?.to_string(),
        "original-registration-cause"
    );
    Ok(())
}
