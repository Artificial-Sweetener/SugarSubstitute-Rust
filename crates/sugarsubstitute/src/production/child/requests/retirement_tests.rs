// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Test actual fixed-ledger retirement ordering with deterministic external wire facts.
use super::*;
#[test]
fn acquisition_retirement_fences_exact_cutoff_without_late_ordinary_authority() -> TestResult {
    let mut r = requests()?;
    let h = prepare(&mut r)?;
    r.acquisition(h, sugarsubstitute_application::startup::ConnectionBudgetKind::Attached)?;
    r.admitted(h)?;
    assert_eq!(r.received(h, Moment::from_nanos(500)), Err(RequestError::Deadline));
    assert!(r.retire(h, Purpose::Acquisition)?);
    assert!(!r.retire(h, Purpose::Acquisition)?);
    r.received_or_retiring(h, Moment::from_nanos(501))?;
    assert_eq!(r.progress(h, Moment::from_nanos(501)), Err(RequestError::Order));
    assert_eq!(r.consumed(h), Err(RequestError::Order));
    assert_eq!(
        r.outbound(h, Moment::from_nanos(500), Moment::from_nanos(499)),
        Err(RequestError::Order)
    );
    assert_eq!(r.find(h)?.original, Moment::from_nanos(500));
    assert_eq!(r.find(h)?.header, h);
    assert_eq!(r.find(h)?.phase, Phase::RetiringReceived);
    Ok(())
}
#[test]
fn acquisition_retirement_keeps_queued_receipt_slot_and_other_owner_bounds() -> TestResult {
    let mut r = requests()?;
    let h = prepare(&mut r)?;
    r.acquisition(h, sugarsubstitute_application::startup::ConnectionBudgetKind::Attached)?;
    r.admitted(h)?;
    r.received(h, Moment::from_nanos(499))?;
    r.retire(h, Purpose::Acquisition)?;
    let output = prepare(&mut r)?;
    r.output_pull(output)?;
    r.admitted(output)?;
    r.retire(output, Purpose::OutputPull)?;
    assert_eq!(r.received_or_retiring(h, Moment::from_nanos(600)), Err(RequestError::Order));
    assert_eq!(r.can_settle(output, Purpose::Acquisition), Err(RequestError::Order));
    assert_eq!(
        r.retire(WireHeader { deadline_qpc: h.deadline_qpc + 1, ..h }, Purpose::Acquisition),
        Err(RequestError::Identity)
    );
    for _ in 2..REQUEST_SLOTS {
        prepare(&mut r)?;
    }
    assert_eq!(
        prepare(&mut r).err().and_then(|e| e.downcast::<RequestError>().ok()).map(|e| *e),
        Some(RequestError::Capacity)
    );
    r.settled(h, Purpose::Acquisition)?;
    let next = prepare(&mut r)?;
    assert_eq!(next.request.map(RequestId::get), Some(32));
    assert!(r.retiring(output)?);
    Ok(())
}
#[test]
fn acquisition_local_withdrawal_requires_prepared_or_genuine_returned_no_issue_phase() -> TestResult
{
    let mut r = requests()?;
    let h = prepare(&mut r)?;
    r.acquisition(h, sugarsubstitute_application::startup::ConnectionBudgetKind::Attached)?;
    let prepared = r.withdraw_acquisition(h)?;
    assert_eq!(prepared.header(), h);
    assert_eq!(prepared.original(), Moment::from_nanos(500));
    assert!(!prepared.native_was_admitted());
    let issued = prepare(&mut r)?;
    r.acquisition(issued, sugarsubstitute_application::startup::ConnectionBudgetKind::Attached)?;
    r.admitted(issued)?;
    assert!(r.withdraw_acquisition(issued).is_err());
    r.retire(issued, Purpose::Acquisition)?;
    assert!(r.withdraw_acquisition(issued).is_err());
    r.revoked_retiring(issued)?;
    let returned = r.withdraw_acquisition(issued)?;
    assert!(returned.native_was_admitted());
    assert_eq!(returned.header(), issued);
    assert_eq!(returned.original(), prepared.original());
    assert!(r.withdraw_acquisition(issued).is_err());
    let output = prepare(&mut r)?;
    r.output_pull(output)?;
    assert!(r.withdraw_acquisition(output).is_err());
    assert_eq!(r.find(output)?.phase, Phase::Prepared);
    Ok(())
}
#[test]
fn retiring_nonfinal_acquire_preserves_original_wait_without_reopening_output_or_grant()
-> TestResult {
    let mut r = requests()?;
    let h = prepare(&mut r)?;
    r.acquisition(h, sugarsubstitute_application::startup::ConnectionBudgetKind::Attached)?;
    r.admitted(h)?;
    r.retire(h, Purpose::Acquisition)?;
    r.received_retiring(h)?;
    r.disposed_progress(h)?;
    assert_eq!(r.find(h)?.phase, Phase::RetiringAdmitted);
    assert_eq!(r.find(h)?.original, Moment::from_nanos(500));
    assert_eq!(r.disposed_progress(h), Err(RequestError::Order));
    r.received_retiring(h)?;
    assert!(r.withdraw_acquisition(h).is_err());
    assert_eq!(r.find(h)?.phase, Phase::RetiringReceived);
    Ok(())
}

#[test]
fn acquisition_deadline_projection_rejects_unrelated_original_errors_without_changing_slots()
-> TestResult {
    let mut r = requests()?;
    let h = prepare(&mut r)?;
    r.acquisition(h, sugarsubstitute_application::startup::ConnectionBudgetKind::Attached)?;
    assert!(matches!(r.expiry(h, RequestError::Order), Err(RequestError::Order)));
    let exact = r.expiry(
        h,
        r.outbound(h, Moment::from_nanos(500), Moment::from_nanos(500))
            .err()
            .ok_or("Real cutoff rejection absent")?,
    )?;
    assert_eq!(exact.header, h);
    assert_eq!(exact.original, Moment::from_nanos(500));
    assert_eq!(exact.purpose, Purpose::Acquisition);
    assert_eq!(
        exact.source().and_then(|e| e.downcast_ref::<RequestError>()),
        Some(&RequestError::Deadline)
    );
    let ordinary = prepare(&mut r)?;
    assert!(matches!(r.expiry(ordinary, RequestError::Deadline), Err(RequestError::Deadline)));
    assert_eq!(r.find(h)?.phase, Phase::Prepared);
    assert_eq!(r.find(ordinary)?.phase, Phase::Prepared);
    Ok(())
}
#[test]
fn acquisition_expired_retained_loan_adoption_preserves_queue_order_and_final_correlation()
-> TestResult {
    let mut r = requests()?;
    let h = prepare(&mut r)?;
    r.acquisition(h, sugarsubstitute_application::startup::ConnectionBudgetKind::Attached)?;
    r.admitted(h)?;
    assert_eq!(r.received(h, Moment::from_nanos(500)), Err(RequestError::Deadline));
    r.retire(h, Purpose::Acquisition)?;
    r.retain_retiring_receipt(h)?;
    assert_eq!(r.find(h)?.phase, Phase::RetiringReceived);
    r.retain_retiring_receipt(h)?;
    assert_eq!(r.consumed(h), Err(RequestError::Order));
    r.disposed_progress(h)?;
    assert_eq!(r.find(h)?.phase, Phase::RetiringAdmitted);
    r.received_retiring(h)?;
    assert_eq!(r.find(h)?.original, Moment::from_nanos(500));
    Ok(())
}
