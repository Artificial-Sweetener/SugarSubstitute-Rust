// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify output ordering and actual owned retirement with controlled external pipes.
use super::*;
use crate::runtime_test_support::{Gate, Rig, TestResult, spec};
use std::{
    num::NonZeroUsize,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::execution::*;
#[path = "test_support.rs"]
mod kernel;
use kernel::*;
#[path = "launch_tests.rs"]
mod launch;
#[path = "ordering_tests.rs"]
mod ordering;
#[path = "pending_tests.rs"]
mod pending;
#[path = "retirement_tests.rs"]
mod retirement;
#[path = "shutdown_tail_tests.rs"]
mod shutdown_tail;
#[test]
fn both_hidden_streams_drain_with_bounded_payload_and_exact_interleaved_gaps() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new();
    for i in 0..2 {
        for _ in 0..40 {
            lock(&boundary.data)[i].push_back(vec![b'a' + i as u8; MAX_BATCH_BYTES]);
        }
        boundary.eof[i].store(true, Ordering::Release);
    }
    let mut session = session(&rig, &boundary)?;
    let JobOutcome::Success(report) = completion(&rig, &mut session)?.outcome else {
        return Err("Expected actual dual EOF".into());
    };
    assert_eq!(report.eof, [true, true]);
    assert!(report.retired);
    assert_eq!(report.queued_chunks, 32);
    assert_eq!(report.queued_bytes, MAX_WINDOWS_OUTPUT_QUEUED_BYTES as u64);
    for counter in report.counters {
        assert_eq!(counter.read_bytes, 40 * MAX_BATCH_BYTES as u64);
        assert_eq!(counter.dropped_bytes, 24 * MAX_BATCH_BYTES as u64);
        assert_eq!(counter.dropped_batches, 24);
    }
    let mut last = [0u64; 2];
    let mut ended = [false; 2];
    while let Some(guard) = session.control.take_batch()? {
        let batch = guard.batch().ok_or("Missing guarded batch")?;
        let i = batch.stream().index();
        assert_eq!(
            batch.sequence().get(),
            last[i] + 1 + batch.gap().map_or(0, OutputGap::skipped_batches)
        );
        last[i] = batch.sequence().get();
        ended[i] |= batch.end();
        assert!(batch.bytes().len() <= MAX_BATCH_BYTES);
    }
    assert_eq!(last, [41, 41]);
    assert_eq!(ended, [true, true]);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    let order = lock(&boundary.order);
    assert!(order.windows(4).all(|turn| turn.iter().filter(|i| **i == 0).count() <= 3));
    Ok(())
}
#[test]
fn cancelled_launch_retains_stdio_pin_and_foreign_generation_never_borrows_handles() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &boundary)?;
    let pin = session.control.take_child_stdio()?;
    assert!(session.control.matches_child_stdio(&pin));
    let mut foreign = resource()?;
    foreign.generation = foreign.generation.next()?;
    assert_eq!(
        pin.handles(OutputIdentity::Managed(foreign)),
        Err(WindowsOutputError::ForeignIdentity)
    );
    assert_eq!(pin.handles(OutputIdentity::Managed(resource()?)), Ok([11, 12, 13]));
    session.control.close();
    boundary.retiring.wait()?;
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(boundary.closes.load(Ordering::Acquire), 0);
    drop(pin);
    assert!(matches!(completion(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    assert_eq!(session.control.snapshot().eof, [false, false]);
    Ok(())
}
#[test]
fn releasing_stdio_pin_preserves_reading_and_actual_eof_after_guarded_delivery() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &boundary)?;
    let pin = session.control.take_child_stdio()?;
    drop(pin);
    boundary.push(0, b"still draining".to_vec())?;
    let guard = loop {
        let count = rig.wake.count();
        if let Some(guard) = session.control.take_batch()? {
            break guard;
        }
        rig.client.drain_completions(NonZeroUsize::MIN)?;
        rig.wake.wait_after(count)?;
    };
    assert_eq!(guard.batch().ok_or("Missing guard")?.bytes(), b"still draining");
    assert_eq!(session.control.take_batch().err(), Some(WindowsOutputError::LoanBusy));
    for _ in 0..40 {
        boundary.push(1, vec![b'e'; MAX_BATCH_BYTES])?;
    }
    boundary.end(0)?;
    boundary.end(1)?;
    let JobOutcome::Success(report) = completion(&rig, &mut session)?.outcome else {
        return Err("Output pin release must not cancel".into());
    };
    assert_eq!(report.eof, [true, true]);
    assert_eq!(boundary.releases.load(Ordering::Acquire), 1);
    assert_eq!(report.counters[1].read_bytes, 40 * MAX_BATCH_BYTES as u64);
    assert!(report.counters[1].dropped_bytes > 0);
    drop(guard);
    assert!(session.control.take_batch()?.is_some());
    Ok(())
}
#[test]
fn cancelled_tail_loss_remains_visible_without_fabricating_eof() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &boundary)?;
    {
        let mut s = lock(&session.control.shared.state);
        s.push(OutputStream::Stdout, b"x")?;
        for _ in 0..32 {
            s.push(OutputStream::Stderr, b"e")?;
        }
    }
    session.control.close();
    assert!(matches!(completion(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    let mut stderr = 0;
    let mut loss = false;
    while let Some(guard) = session.control.take_batch()? {
        let batch = guard.batch().ok_or("Missing guard")?;
        assert!(!batch.end());
        if batch.stream() == OutputStream::Stderr {
            stderr += 1;
            assert_eq!(batch.sequence().get(), stderr);
        } else {
            assert_eq!(batch.sequence().get(), 2);
            assert_eq!(batch.gap().ok_or("Missing loss")?.dropped_bytes(), 1);
            assert_eq!(batch.gap().ok_or("Missing loss")?.skipped_batches(), 1);
            assert!(batch.bytes().is_empty());
            loss = true;
        }
    }
    assert_eq!(stderr, 32);
    assert!(loss);
    assert_eq!(session.control.snapshot().eof, [false, false]);
    Ok(())
}
#[test]
fn partial_native_creation_and_failed_retirement_preserve_original_cause_until_late_close()
-> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new();
    boundary.fail_prepare.store(true, Ordering::Release);
    boundary.fail_retire.store(1, Ordering::Release);
    let _retirement = RetirementHold::new(&boundary);
    let scope = rig.client.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(2)?, deadline: None },
        None,
        true,
    )?;
    let mut specification = spec(1, JobClass::NativeObservation, MIN_WINDOWS_OUTPUT_BYTES)?;
    specification.operation = Operation::NativeLifecycle;
    let mut session = reserve(
        &rig.client.core,
        &scope,
        specification,
        OutputIdentity::Managed(resource()?),
        Box::new(Lease(boundary.clone())),
    )?;
    boundary.retiring.wait()?;
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(boundary.closes.load(Ordering::Acquire), 0);
    boundary.allow_retire.store(true, Ordering::Release);
    boundary.wake(0)?;
    let JobOutcome::Failure(error) = completion(&rig, &mut session)?.outcome else {
        return Err("Expected partial creation cause".into());
    };
    assert_eq!(error.stage(), "partial_output_prepare");
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn pending_issue_panic_keeps_admitted_storage_through_blocked_retirement() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new();
    let _retirement = RetirementHold::new(&boundary);
    let mut session = session(&rig, &boundary)?;
    boundary.panic_read.store(true, Ordering::Release);
    boundary.wake(0)?;
    boundary.retiring.wait()?;
    assert!(session.completion.try_take_completion().is_none());
    assert!(lock(&rig.client.core.state).bytes >= MIN_WINDOWS_OUTPUT_BYTES);
    boundary.allow_retire.store(true, Ordering::Release);
    boundary.wake(0)?;
    assert!(matches!(completion(&rig, &mut session)?.outcome, JobOutcome::Panicked(_)));
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn actual_output_loan_release_wakes_parked_consumers_after_native_worker_has_retired() -> TestResult
{
    let rig = Rig::new()?;
    let boundary = Boundary::new();
    lock(&boundary.data)[0].push_back(b"first".to_vec());
    lock(&boundary.data)[0].push_back(b"second".to_vec());
    boundary.eof[0].store(true, Ordering::Release);
    boundary.eof[1].store(true, Ordering::Release);
    let mut session = session(&rig, &boundary)?;
    assert!(matches!(completion(&rig, &mut session)?.outcome, JobOutcome::Success(_)));
    assert!(session.control.snapshot().retired);
    let guard = session.control.take_batch()?.ok_or("First batch absent")?;
    assert_eq!(guard.batch().ok_or("Managed batch absent")?.bytes(), b"first");
    assert!(!session.control.loan_available());
    assert_eq!(session.control.take_batch().err(), Some(WindowsOutputError::LoanBusy));
    let retained = session.control.snapshot();
    rig.client.drain_completions(NonZeroUsize::new(32).ok_or("Drain bound absent")?)?;
    let observed = rig.wake.count();
    drop(guard);
    assert!(rig.wake.count() > observed);
    assert!(session.control.loan_available());
    let next = session.control.take_batch()?.ok_or("Second batch absent")?;
    let batch = next.batch().ok_or("Second managed batch absent")?;
    assert_eq!(batch.bytes(), b"second");
    assert_eq!(batch.sequence().get(), 2);
    assert_eq!(batch.gap(), None);
    assert_eq!(session.control.snapshot().counters[0].read_bytes, retained.counters[0].read_bytes);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    drop(next);
    Ok(())
}
