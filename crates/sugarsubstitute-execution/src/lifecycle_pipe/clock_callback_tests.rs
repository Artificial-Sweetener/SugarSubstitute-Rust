// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reenter actual execution controls from the supplied clock before the current issue fence.
use super::*;
struct Release {
    control: Arc<LifecyclePipeControl>,
    boundary: Arc<Boundary>,
}
impl Drop for Release {
    fn drop(&mut self) {
        self.control.close();
        self.boundary.allow_retire.store(true, Ordering::Release);
        self.boundary.write_release.open();
        if let Some(signals) = lock(&self.boundary.signals).as_ref() {
            signals[Role::Handshake.index()].complete();
        }
    }
}
fn arm(
    rig: &Rig,
    boundary: &Boundary,
    control: &Arc<LifecyclePipeControl>,
    action: crate::runtime_test_support::ClockCallback,
) -> (Arc<AtomicBool>, Arc<Gate>) {
    let unlocked = Arc::new(AtomicBool::new(false));
    let observed = Arc::new(Gate::default());
    let result = unlocked.clone();
    let arrived = observed.clone();
    let shared = control.inner.shared.clone();
    let clock = rig.clock.clone();
    *lock(&boundary.clock_probe) = Some((
        Role::Write,
        Box::new(move || {
            clock.on_next_current_thread_observation(Box::new(move || {
                let lifetime_free = shared.state.try_lock().is_ok();
                result.store(lifetime_free, Ordering::Release);
                if lifetime_free {
                    action();
                }
                arrived.open();
            }));
        }),
    ));
    (unlocked, observed)
}
#[test]
fn supplied_clock_revokes_exact_write_and_returns_original_buffer_without_issue() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    connect(&rig, &session)?;
    boundary.hold_write.store(true, Ordering::Release);
    let mut output = session.control.take_write_buffer()?.ok_or("Missing original buffer")?;
    output.as_mut_bytes()[..5].copy_from_slice(b"hello");
    let address = output.as_mut_bytes().as_ptr();
    let admission = Arc::new(
        session
            .control
            .write(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900), output, 5)
            .map_err(|rejection| rejection.error)?,
    );
    boundary.preparing_write.wait()?;
    let control = Arc::new(session.control);
    let _release = Release { control: control.clone(), boundary: boundary.clone() };
    let revocation = Arc::new(Mutex::new(None));
    let saved = revocation.clone();
    let original = admission.clone();
    let (unlocked, observed) = arm(
        &rig,
        &boundary,
        &control,
        Box::new(move || {
            *lock(&saved) = Some(original.revoke());
        }),
    );
    boundary.write_release.open();
    observed.wait()?;
    assert!(unlocked.load(Ordering::Acquire));
    assert_eq!(*lock(&revocation), Some(NativeWriteRevocation::RevokedBeforeIssue));
    let LifecyclePipeObservation::WriteRevoked { operation, mut buffer } =
        fact(&rig, &control, |fact| matches!(fact, LifecyclePipeObservation::WriteRevoked { .. }))?
    else {
        return Err("Original veto result missing".into());
    };
    assert_eq!(operation.get(), 1);
    assert_eq!(buffer.as_mut_bytes().as_ptr(), address);
    assert_eq!(&buffer.as_mut_bytes()[..5], b"hello");
    assert_eq!(boundary.writes.load(Ordering::Acquire), 0);
    assert!(control.take_observation().is_none());
    assert_eq!(admission.revoke(), NativeWriteRevocation::AlreadyRevoked);
    drop(buffer);
    control.close();
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Cancelled));
    Ok(())
}
#[test]
fn supplied_clock_closes_current_lifetime_and_retains_exact_memory_until_checked_retirement()
-> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    connect(&rig, &session)?;
    boundary.hold_write.store(true, Ordering::Release);
    boundary.allow_retire.store(false, Ordering::Release);
    let output = session.control.take_write_buffer()?.ok_or("Missing original buffer")?;
    session
        .control
        .write(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900), output, 1)
        .map_err(|rejection| rejection.error)?;
    boundary.preparing_write.wait()?;
    let control = Arc::new(session.control);
    let _release = Release { control: control.clone(), boundary: boundary.clone() };
    let closing = control.clone();
    let (unlocked, observed) = arm(&rig, &boundary, &control, Box::new(move || closing.close()));
    boundary.write_release.open();
    observed.wait()?;
    assert!(unlocked.load(Ordering::Acquire));
    boundary.retiring.wait()?;
    assert_eq!(boundary.writes.load(Ordering::Acquire), 0);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 0);
    assert!(control.take_observation().is_none());
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(scope.snapshot().reserved_bytes, MIN_LIFECYCLE_PIPE_BYTES);
    assert!(
        lock(&boundary.memory).as_ref().ok_or("Missing native owner memory")?.claim(1).is_err()
    );
    boundary.allow_retire.store(true, Ordering::Release);
    boundary.wake(Role::Handshake)?;
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Cancelled));
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn supplied_clock_cancels_actual_scope_without_native_issue_or_early_memory_release() -> TestResult
{
    let rig = Rig::new()?;
    let scope = Arc::new(scope(&rig)?);
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    connect(&rig, &session)?;
    boundary.hold_write.store(true, Ordering::Release);
    boundary.allow_retire.store(false, Ordering::Release);
    let output = session.control.take_write_buffer()?.ok_or("Missing original buffer")?;
    session
        .control
        .write(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900), output, 1)
        .map_err(|rejection| rejection.error)?;
    boundary.preparing_write.wait()?;
    let control = Arc::new(session.control);
    let _release = Release { control: control.clone(), boundary: boundary.clone() };
    let cancelling = scope.clone();
    let (unlocked, observed) =
        arm(&rig, &boundary, &control, Box::new(move || cancelling.cancel()));
    boundary.write_release.open();
    observed.wait()?;
    assert!(unlocked.load(Ordering::Acquire));
    boundary.retiring.wait()?;
    assert_eq!(boundary.writes.load(Ordering::Acquire), 0);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 0);
    assert!(control.take_observation().is_none());
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(scope.snapshot().reserved_bytes, MIN_LIFECYCLE_PIPE_BYTES);
    assert!(
        lock(&boundary.memory).as_ref().ok_or("Missing native owner memory")?.claim(1).is_err()
    );
    boundary.allow_retire.store(true, Ordering::Release);
    boundary.wake(Role::Handshake)?;
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Cancelled));
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn supplied_clock_deadline_equality_is_revalidated_without_native_effect_or_rearming() -> TestResult
{
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    connect(&rig, &session)?;
    boundary.hold_write.store(true, Ordering::Release);
    let output = session.control.take_write_buffer()?.ok_or("Missing original buffer")?;
    session
        .control
        .write(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900), output, 1)
        .map_err(|rejection| rejection.error)?;
    boundary.preparing_write.wait()?;
    let control = Arc::new(session.control);
    let _release = Release { control: control.clone(), boundary: boundary.clone() };
    let clock = rig.clock.clone();
    let (unlocked, observed) = arm(&rig, &boundary, &control, Box::new(move || clock.advance(900)));
    boundary.write_release.open();
    observed.wait()?;
    assert!(unlocked.load(Ordering::Acquire));
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::TimedOut));
    assert_eq!(boundary.writes.load(Ordering::Acquire), 0);
    assert!(control.take_observation().is_none());
    assert_eq!(control.original_deadline(), Moment::from_nanos(1000));
    Ok(())
}

#[test]
fn same_number_context_from_foreign_execution_owner_cannot_issue_actual_write() -> TestResult {
    let alien = Rig::new()?;
    let (_alien_scope, context) = crate::runtime_test_support::admitted_context(&alien, 2, 1)?;
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    assert_ne!(scope.owner_id(), context.owner_id());
    assert_eq!(scope.id(), context.scope_id());
    let boundary = Boundary::new();
    let mut session = session(&rig, &scope, &boundary, true)?;
    connect(&rig, &session)?;
    boundary.hold_write.store(true, Ordering::Release);
    let output = session.control.take_write_buffer()?.ok_or("Missing original buffer")?;
    session
        .control
        .write(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900), output, 1)
        .map_err(|rejection| rejection.error)?;
    boundary.preparing_write.wait()?;
    let control = Arc::new(session.control);
    let _release = Release { control: control.clone(), boundary: boundary.clone() };
    *lock(&boundary.context_override) = Some(context);
    boundary.write_release.open();
    boundary.writing.wait()?;
    assert_eq!(boundary.writes.load(Ordering::Acquire), 0);
    assert!(control.take_observation().is_none());
    control.close();
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::Cancelled));
    Ok(())
}
