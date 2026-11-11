// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check admission, exact provenance, stop checkpoints and atomic bounded loss.
use super::*;

#[test]
fn rejected_reservations_do_not_create_native_state_or_consume_the_job_identity() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new();
    let scope = rig.client.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(2)?, deadline: None },
        None,
        true,
    )?;
    let mut good = spec(1, JobClass::NativeObservation, MIN_WINDOWS_OUTPUT_BYTES)?;
    good.operation = Operation::NativeLifecycle;
    let mut too_small = good;
    too_small.reservation_bytes -= 1;
    assert!(matches!(
        reserve(
            &rig.client.core,
            &scope,
            too_small,
            OutputIdentity::Managed(resource()?),
            Box::new(Lease(boundary.clone()))
        ),
        Err(AdmissionError::NativeReservationTooSmall { .. })
    ));
    assert!(matches!(
        reserve(
            &rig.client.core,
            &rig.scope,
            good,
            OutputIdentity::Managed(resource()?),
            Box::new(Lease(boundary.clone()))
        ),
        Err(AdmissionError::NativeScopeRequired)
    ));
    let mut wrong = good;
    wrong.class = JobClass::InfrastructureLifecycle;
    assert!(matches!(
        reserve(
            &rig.client.core,
            &scope,
            wrong,
            OutputIdentity::Managed(resource()?),
            Box::new(Lease(boundary.clone()))
        ),
        Err(AdmissionError::WrongClass { .. })
    ));
    assert_eq!(boundary.prepares.load(Ordering::Acquire), 0);
    assert_eq!(lock(&rig.client.core.state).bytes, 0);
    let mut session = reserve(
        &rig.client.core,
        &scope,
        good,
        OutputIdentity::Managed(resource()?),
        Box::new(Lease(boundary.clone())),
    )?;
    session.control.close();
    assert!(matches!(completion(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    Ok(())
}

#[test]
fn same_resource_does_not_make_another_sessions_stdio_provenance_valid() -> TestResult {
    let rig = Rig::new()?;
    let first = Boundary::new();
    let second = Boundary::new();
    let mut a = session(&rig, &first)?;
    let mut b = session_with(&rig, &second, 3, 2, None)?;
    let pin = a.control.take_child_stdio()?;
    assert_eq!(a.control.snapshot().identity, b.control.snapshot().identity);
    assert!(a.control.matches_child_stdio(&pin));
    assert!(!b.control.matches_child_stdio(&pin));
    assert_eq!(a.control.take_child_stdio().err(), Some(WindowsOutputError::AlreadyTransferred));
    drop(pin);
    a.control.close();
    b.control.close();
    assert!(matches!(completion(&rig, &mut a)?.outcome, JobOutcome::Cancelled));
    assert!(matches!(completion(&rig, &mut b)?.outcome, JobOutcome::Cancelled));
    Ok(())
}

#[test]
fn deadline_crossed_by_stdout_completion_prevents_the_later_stderr_issue() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new();
    lock(&boundary.data)[0].push_back(b"actual stdout".to_vec());
    lock(&boundary.data)[1].push_back(b"must not issue".to_vec());
    *lock(&boundary.advance) = Some((rig.clock.clone(), 100));
    let mut session = session_with(&rig, &boundary, 2, 1, Some(Moment::from_nanos(100)))?;
    assert!(matches!(completion(&rig, &mut session)?.outcome, JobOutcome::TimedOut));
    assert_eq!(*lock(&boundary.order), vec![0]);
    let guard = session.control.take_batch()?.ok_or("Actual completed stdout was lost")?;
    assert_eq!(guard.batch().ok_or("Missing batch")?.bytes(), b"actual stdout");
    assert!(!guard.batch().ok_or("Missing batch")?.end());
    Ok(())
}

#[test]
fn sequence_and_counter_exhaustion_reject_without_mutating_retained_output() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &boundary)?;
    {
        let mut s = lock(&session.control.shared.state);
        s.sequences[0] = u64::MAX;
        assert_eq!(
            s.push(OutputStream::Stdout, b"x").err().ok_or("Missing exhaustion")?.stage(),
            "output_sequence_exhausted"
        );
        assert_eq!(s.counters[0].read_bytes, 0);
        s.sequences[0] = 0;
        s.counters[0].read_bytes = u64::MAX;
        assert_eq!(
            s.push(OutputStream::Stdout, b"x").err().ok_or("Missing overflow")?.stage(),
            "output_counter_overflow"
        );
        assert_eq!(s.sequences[0], 0);
        s.counters[0].read_bytes = 0;
        s.push(OutputStream::Stdout, b"preserved")?;
        for _ in 0..31 {
            s.push(OutputStream::Stderr, b"e")?;
        }
        s.counters[0].dropped_bytes = u64::MAX;
        assert!(s.push(OutputStream::Stderr, b"replacement").is_err());
        assert_eq!(s.queue.len(), 32);
        assert_eq!(s.queue.front().ok_or("Missing retained front")?.buffer.bytes, b"preserved");
        assert_eq!(s.tail[0].bytes, 0);
    }
    session.control.close();
    assert!(matches!(completion(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    Ok(())
}

#[test]
fn delivered_guard_retains_real_admission_after_native_terminal_and_control_drop() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new();
    lock(&boundary.data)[0].push_back(b"charged bytes".to_vec());
    boundary.eof[0].store(true, Ordering::Release);
    boundary.eof[1].store(true, Ordering::Release);
    let mut session = session(&rig, &boundary)?;
    assert!(matches!(completion(&rig, &mut session)?.outcome, JobOutcome::Success(_)));
    let guard = session.control.take_batch()?.ok_or("Missing guarded source")?;
    drop(session);
    assert!(lock(&rig.client.core.state).bytes >= MIN_WINDOWS_OUTPUT_BYTES);
    assert_eq!(guard.batch().ok_or("Missing guarded batch")?.bytes(), b"charged bytes");
    drop(guard);
    Ok(())
}
