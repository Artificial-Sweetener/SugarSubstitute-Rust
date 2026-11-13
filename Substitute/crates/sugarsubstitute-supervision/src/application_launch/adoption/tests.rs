// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real stand-in children adopt, exit or stay silent; the creator's outcome follows what it saw.
use super::*;
use crate::{
    application_launch::engine::tests::stand_in,
    setup_storage::test_worker::{self as worker, TestResult, driven::run_driven},
};
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::{Foundation::WAIT_OBJECT_0, System::Threading::WaitForSingleObject};

const BYTES: u64 = 1024 * 1024;
const BOUNDS: Bounds = Bounds {
    adoption: Duration::from_secs(10),
    grace: Duration::from_secs(2),
    termination: Duration::from_secs(10),
};

/// Whether the stand-in's process has exited, observed without waiting.
fn exited(process: BorrowedHandle<'_>) -> bool {
    // SAFETY: the started child keeps its process handle live; a zero-timeout wait only observes.
    let state = unsafe { WaitForSingleObject(HANDLE(process.as_raw_handle()), 0) };
    state == WAIT_OBJECT_0
}

#[test]
fn a_child_holding_its_slot_is_adopted_and_keeps_the_handoff_until_its_creator_leaves() -> TestResult
{
    worker::run_reserved(BYTES, |context, _| {
        let child = stand_in("signal", context)?;
        let (event, process) = child.handles();
        let released = AtomicBool::new(false);
        let adoption = await_adoption(
            context,
            event,
            process,
            || released.store(true, Ordering::SeqCst),
            BOUNDS,
        );
        assert!(matches!(adoption, Adoption::Adopted), "{adoption:?}");
        assert!(!released.load(Ordering::SeqCst));
        Ok(())
    })
}

#[test]
fn a_child_exiting_before_it_holds_its_slot_is_unadopted_without_waiting_for_any_bound()
-> TestResult {
    worker::run_reserved(BYTES, |context, _| {
        let child = stand_in("exit", context)?;
        let (event, process) = child.handles();
        let released = AtomicBool::new(false);
        let adoption = await_adoption(
            context,
            event,
            process,
            || released.store(true, Ordering::SeqCst),
            BOUNDS,
        );
        assert!(matches!(adoption, Adoption::Unadopted(UnadoptedEnding::Exited)), "{adoption:?}");
        assert!(!released.load(Ordering::SeqCst));
        assert!(exited(process));
        Ok(())
    })
}

/// Antivirus holding the new files keeps the child from ever reaching its entry. At its bound
/// the creator gives up the handoff, waits out the grace, ends the child and sees it gone.
#[test]
fn a_silent_child_loses_its_handoff_and_is_ended_after_the_grace() -> TestResult {
    let released = std::sync::Arc::new(AtomicBool::new(false));
    let observed = released.clone();
    run_driven(
        BYTES,
        move |context, handoff| {
            let child = stand_in("hang", context)?;
            let (event, process) = child.handles();
            handoff.reach(1);
            let adoption = await_adoption(
                context,
                event,
                process,
                || observed.store(true, Ordering::SeqCst),
                BOUNDS,
            );
            assert!(
                matches!(adoption, Adoption::Unadopted(UnadoptedEnding::Ended)),
                "{adoption:?}"
            );
            assert!(observed.load(Ordering::SeqCst));
            assert!(exited(process));
            Ok(())
        },
        |driver| {
            driver.handoff.wait(1);
            let bound = Moment::from_nanos(BOUNDS.adoption.as_nanos() as u64);
            driver.clock.wait_for_sleeper(bound);
            driver.clock.advance_to(bound)?;
            let grace = bound.checked_add(BOUNDS.grace)?;
            driver.clock.wait_for_sleeper(grace);
            driver.clock.advance_to(grace)?;
            Ok(())
        },
    )?;
    assert!(released.load(Ordering::SeqCst));
    Ok(())
}

/// A cancelled creator cannot tell whether its child took over: it gives up the handoff, so the
/// child can no longer adopt from it, and reports the creation as uncertain.
#[test]
fn a_cancelled_wait_gives_up_the_handoff_and_reports_uncertain_creation() -> TestResult {
    run_driven(
        BYTES,
        |context, handoff| {
            let child = stand_in("hang", context)?;
            let (event, process) = child.handles();
            handoff.reach(1);
            let released = AtomicBool::new(false);
            let adoption = await_adoption(
                context,
                event,
                process,
                || released.store(true, Ordering::SeqCst),
                BOUNDS,
            );
            assert!(matches!(adoption, Adoption::Uncertain(_)), "{adoption:?}");
            assert!(released.load(Ordering::SeqCst));
            // SAFETY: this test's own hung stand-in; nothing else owns or observes it.
            unsafe { TerminateProcess(HANDLE(process.as_raw_handle()), 1) }?;
            Ok(())
        },
        |driver| {
            driver.handoff.wait(1);
            driver.clock.wait_for_sleeper(Moment::from_nanos(BOUNDS.adoption.as_nanos() as u64));
            driver.cancel();
            Ok(())
        },
    )
}
