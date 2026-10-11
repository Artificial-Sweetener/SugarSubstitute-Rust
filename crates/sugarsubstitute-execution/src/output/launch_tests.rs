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
    assert_eq!(cancel.resource(), resource()?);
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
