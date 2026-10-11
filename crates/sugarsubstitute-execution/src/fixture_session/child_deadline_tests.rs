// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve admitted child deadlines through actual cancellation and late binds.
use super::*;
#[test]
fn deadline_translation_floors_and_checks_epoch_frequency_and_overflow() -> TestResult {
    let deadline = FixtureDeadline::new(3, 3)?;
    assert_eq!(translated(Moment::from_nanos(10), 1, 3, deadline)?.as_nanos(), 666_666_676);
    assert!(translated(Moment::from_nanos(10), 0, 3, deadline).is_err());
    assert!(translated(Moment::from_nanos(10), 1, 2, deadline).is_err());
    assert!(translated(Moment::from_nanos(10), 3, 3, deadline).is_err());
    assert!(translated(Moment::from_nanos(u64::MAX), 1, 3, deadline).is_err());
    assert!(FixtureDeadline::new(0, 3).is_err());
    assert!(FixtureDeadline::new(3, 0).is_err());
    Ok(())
}

#[test]
fn local_clock_is_sampled_before_qpc_and_admission_keeps_derived_deadline() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let permit = reserve_sampled(
        &rig.client.core,
        &scope,
        spec()?,
        NativeCorrelation::new(1)?,
        FixtureDeadline::new(2, 1)?,
        || {
            rig.clock.advance(100);
            Ok((1, 1))
        },
    )?;
    assert_eq!(permit.deadline(), Moment::from_nanos(1_000_000_000));
    rig.clock.advance(100);
    assert_eq!(permit.deadline(), Moment::from_nanos(1_000_000_000));
    let mut job = permit.cancel_unbound();
    assert_eq!(job.spec().deadline, Some(Moment::from_nanos(1_000_000_000)));
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::Cancelled));
    Ok(())
}

#[test]
fn late_bind_preserves_prebind_deadline_and_actual_typed_retirement() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new(false, false);
    let permit = permit(&rig, &scope)?;
    let original = permit.deadline();
    rig.clock.advance(100);
    assert_eq!(permit.deadline(), original);
    let mut job = permit.bind_lease(Box::new(Lease(boundary.clone())));
    assert_eq!(job.spec().deadline, Some(original));
    boundary.ready.wait()?;
    boundary.signal(false)?;
    boundary.response.wait()?;
    boundary.signal(true)?;
    let completion = rig.completion(&mut job)?;
    assert_eq!(completion.spec.deadline, Some(original));
    assert!(matches!(completion.outcome, JobOutcome::Success(report) if report.response_signalled));
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn clock_or_expired_admission_retains_caller_input_and_consumes_no_identity() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let input = InheritedFixtureEvents::new(11, 12, 13, 14)?;
    assert!(matches!(
        reserve_sampled(
            &rig.client.core,
            &scope,
            spec()?,
            NativeCorrelation::new(1)?,
            FixtureDeadline::new(2, 1)?,
            || Err(FixtureChildAdmissionError::Clock(NativeObservationError::new(
                "fixture_qpc",
                std::io::Error::from_raw_os_error(5)
            )))
        ),
        Err(FixtureChildAdmissionError::Clock(_))
    ));
    assert!(
        reserve_sampled(
            &rig.client.core,
            &scope,
            spec()?,
            NativeCorrelation::new(1)?,
            FixtureDeadline::new(2, 1)?,
            || Ok((2, 1))
        )
        .is_err()
    );
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    assert_eq!(input.handles, [11, 12, 13, 14]);
    let mut job = permit(&rig, &scope)?.cancel_unbound();
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::Cancelled));
    Ok(())
}
