// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve only actual late read facts after cancellation and checked retirement.
use super::*;

#[test]
fn cancelled_pending_read_retains_actual_bytes_only_after_checked_retirement() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new();
    boundary.defer_read[0].store(true, Ordering::Release);
    let _retirement = RetirementHold::new(&boundary);
    lock(&boundary.data)[0].push_back(b"completed before cancel".to_vec());
    let mut session = session(&rig, &boundary)?;
    boundary.issued.wait()?;
    session.control.close();
    boundary.retiring.wait()?;
    assert!(session.control.take_batch()?.is_none());
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(boundary.closes.load(Ordering::Acquire), 0);
    boundary.allow_retire.store(true, Ordering::Release);
    boundary.wake(0)?;
    assert!(matches!(completion(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    let guard = session.control.take_batch()?.ok_or("Missing actual completed bytes")?;
    let batch = guard.batch().ok_or("Missing guarded batch")?;
    assert_eq!(batch.bytes(), b"completed before cancel");
    assert_eq!(batch.sequence().get(), 1);
    assert!(!batch.end());
    assert_eq!(session.control.snapshot().eof, [false, false]);
    Ok(())
}

#[test]
fn cancelled_pending_zero_byte_read_is_not_eof_and_aborted_read_has_no_fact() -> TestResult {
    for completed in [None, Some(Progress::Empty)] {
        let rig = Rig::new()?;
        let boundary = Boundary::new();
        let mut session = session(&rig, &boundary)?;
        *lock(&boundary.completed) = [completed, None];
        session.control.close();
        assert!(matches!(completion(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
        assert!(session.control.take_batch()?.is_none());
        assert_eq!(session.control.snapshot().eof, [false, false]);
    }
    Ok(())
}

#[test]
fn actual_broken_pipe_after_cancel_preserves_one_eof_without_success_reclassification() -> TestResult
{
    let rig = Rig::new()?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &boundary)?;
    *lock(&boundary.completed) = [Some(Progress::Eof), None];
    session.control.close();
    assert!(matches!(completion(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    let guard = session.control.take_batch()?.ok_or("Missing actual EOF")?;
    let batch = guard.batch().ok_or("Missing guarded EOF")?;
    assert!(batch.end());
    assert_eq!(batch.stream(), OutputStream::Stdout);
    assert!(batch.bytes().is_empty());
    drop(guard);
    assert!(session.control.take_batch()?.is_none());
    assert_eq!(session.control.snapshot().eof, [true, false]);
    Ok(())
}

#[test]
fn invalid_late_native_count_remains_failure_without_published_bytes_or_eof() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &boundary)?;
    *lock(&boundary.completed) = [Some(Progress::Bytes(MAX_BATCH_BYTES as u32 + 1)), None];
    session.control.close();
    let JobOutcome::Failure(error) = completion(&rig, &mut session)?.outcome else {
        return Err("Invalid native count must remain observable".into());
    };
    assert_eq!(error.stage(), "output_retired_read_count");
    assert!(session.control.take_batch()?.is_none());
    assert_eq!(session.control.snapshot().eof, [false, false]);
    Ok(())
}

struct EndClock {
    clock: crate::runtime_test_support::ClockPort,
    panic_on: Arc<Mutex<Option<std::thread::ThreadId>>>,
}
impl Clock for EndClock {
    fn now(&self) -> Moment {
        let should_panic = {
            let mut target = lock(&self.panic_on);
            if *target == Some(std::thread::current().id()) {
                target.take();
                true
            } else {
                false
            }
        };
        if should_panic {
            std::panic::resume_unwind(Box::new("Controlled final native end-clock panic"));
        }
        self.clock.now()
    }
    fn sleep_until(&self, deadline: Moment) -> WaitFuture {
        self.clock.sleep_until(deadline)
    }
}

#[test]
fn end_clock_panic_after_native_close_still_delivers_typed_terminal_once() -> TestResult {
    use crate::{
        runtime::ExecutionOwner,
        runtime_test_support::{ClockPort, ManualClock, Wake},
    };
    let clock = Arc::new(ManualClock::default());
    let wake = Arc::new(Wake::default());
    let panic_on = Arc::new(Mutex::new(None));
    let owner = ExecutionOwner::with_cpu_workers(
        Arc::new(EndClock { clock: ClockPort(clock.clone()), panic_on: panic_on.clone() }),
        wake.clone(),
        2,
    )?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let rig = Rig { owner, client, clock, wake, scope };
    let boundary = Boundary::new();
    *lock(&boundary.end_clock_panic) = Some(panic_on);
    let mut session = session(&rig, &boundary)?;
    session.control.close();
    let JobOutcome::Panicked(cause) = completion(&rig, &mut session)?.outcome else {
        return Err("Final clock failure must not discard typed terminal".into());
    };
    assert!(cause.as_str().contains("end-clock"));
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    assert!(session.control.snapshot().retired);
    assert!(session.completion.try_take_completion().is_none());
    Ok(())
}
