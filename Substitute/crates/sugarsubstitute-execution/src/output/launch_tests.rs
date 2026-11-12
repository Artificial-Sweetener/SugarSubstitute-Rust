// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise exact pin release and retained cancellation through the real output owner.
use super::*;
#[test]
fn successful_pin_release_and_close_capability_drop_do_not_cancel_drainage() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &boundary)?;
    let pin = session.control.take_child_stdio()?;
    let cancel = pin.release_after_attributes();
    assert_eq!(cancel.identity(), OutputIdentity::Managed(resource()?));
    assert!(session.control.matches_cancellation(&cancel));
    drop(cancel);
    boundary.push(0, b"successful launch continues".to_vec())?;
    boundary.end(0)?;
    boundary.end(1)?;
    assert!(matches!(completion(&rig, &mut session)?.outcome, JobOutcome::Success(_)));
    assert_eq!(boundary.releases.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn pin_close_request_cannot_publish_terminal_before_actual_attribute_release() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &boundary)?;
    let pin = session.control.take_child_stdio()?;
    pin.request_close();
    boundary.retiring.wait()?;
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(boundary.closes.load(Ordering::Acquire), 0);
    let cancel = pin.release_after_attributes();
    assert!(matches!(completion(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    assert_eq!(session.control.snapshot().eof, [false, false]);
    drop(cancel);
    Ok(())
}
#[test]
fn retained_close_capability_cancels_after_parent_copies_already_released() -> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &boundary)?;
    let cancel = session.control.take_child_stdio()?.release_after_attributes();
    cancel.request_close();
    assert!(matches!(completion(&rig, &mut session)?.outcome, JobOutcome::Cancelled));
    assert!(session.control.snapshot().retired);
    assert_eq!(session.control.snapshot().eof, [false, false]);
    Ok(())
}
#[test]
fn cancellation_provenance_rejects_same_resource_from_another_session() -> TestResult {
    let rig = Rig::new()?;
    let first = Boundary::new();
    let second = Boundary::new();
    let mut a = session(&rig, &first)?;
    let mut b = session_with(&rig, &second, 3, 2, None)?;
    let cancel = a.control.take_child_stdio()?.release_after_attributes();
    assert!(a.control.matches_cancellation(&cancel));
    assert!(!b.control.matches_cancellation(&cancel));
    cancel.request_close();
    assert!(matches!(completion(&rig, &mut a)?.outcome, JobOutcome::Cancelled));
    assert!(!b.control.snapshot().retired);
    b.control.close();
    assert!(matches!(completion(&rig, &mut b)?.outcome, JobOutcome::Cancelled));
    Ok(())
}

#[test]
fn desktop_output_retains_exact_run_and_claimed_bytes_through_pin_retirement_and_last_loan()
-> TestResult {
    use sugarsubstitute_contracts::lifecycle::{InstanceId, RunGeneration, RunKey};
    let rig = Rig::new()?;
    let current = RunKey {
        owner: resource()?.owner,
        instance: InstanceId::new(7)?,
        generation: RunGeneration::new(1)?,
    };
    let mut foreign = current;
    foreign.generation = RunGeneration::new(2)?;
    let first = Boundary::new();
    let second = Boundary::new();
    let mut a = session_identity(&rig, &first, 2, 1, None, OutputIdentity::Desktop(current))?;
    let mut b = session_identity(&rig, &second, 3, 2, None, OutputIdentity::Desktop(current))?;
    assert_eq!(MIN_WINDOWS_OUTPUT_BYTES, 1_310_720);
    let pin = a.control.take_child_stdio()?;
    assert_eq!(pin.identity(), OutputIdentity::Desktop(current));
    assert_eq!(
        pin.handles(OutputIdentity::Desktop(foreign)),
        Err(WindowsOutputError::ForeignIdentity)
    );
    assert_eq!(
        pin.handles(OutputIdentity::Managed(resource()?)),
        Err(WindowsOutputError::ForeignIdentity)
    );
    assert!(a.control.matches_child_stdio(&pin));
    assert!(!b.control.matches_child_stdio(&pin));
    first.push(1, b"actual desktop startup stderr".to_vec())?;
    let guard = loop {
        let observed = rig.wake.count();
        rig.client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(guard) = a.control.take_batch()? {
            break guard;
        }
        rig.wake.wait_after(observed)?;
    };
    assert!(guard.batch().is_none());
    let batch = guard.desktop_batch().ok_or("Desktop source payload missing")?;
    assert_eq!(batch.run(), current);
    assert_eq!(batch.stream(), OutputStream::Stderr);
    assert_eq!(batch.sequence().get(), 1);
    assert_eq!(batch.bytes(), b"actual desktop startup stderr");
    assert_eq!(batch.gap(), None);
    assert!(!batch.end());
    assert_eq!(a.control.take_batch().err(), Some(WindowsOutputError::LoanBusy));
    a.control.close();
    first.retiring.wait()?;
    assert!(a.completion.try_take_completion().is_none());
    assert_eq!(first.closes.load(Ordering::Acquire), 0);
    drop(pin);
    assert!(matches!(completion(&rig, &mut a)?.outcome, JobOutcome::Cancelled));
    assert!(a.control.snapshot().retired);
    assert_eq!(a.control.snapshot().identity, OutputIdentity::Desktop(current));
    assert_eq!(a.control.snapshot().eof, [false, false]);
    b.control.close();
    assert!(matches!(completion(&rig, &mut b)?.outcome, JobOutcome::Cancelled));
    drop(a);
    drop(b);
    let scope = lock(&first.scope).take().ok_or("Actual desktop scope missing")?;
    let Rig { owner, .. } = rig;
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, MIN_WINDOWS_OUTPUT_BYTES);
    assert_eq!(
        guard.desktop_batch().ok_or("Retired source lost")?.bytes(),
        b"actual desktop startup stderr"
    );
    drop(guard);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    assert_eq!(first.closes.load(Ordering::Acquire), 1);
    Ok(())
}
