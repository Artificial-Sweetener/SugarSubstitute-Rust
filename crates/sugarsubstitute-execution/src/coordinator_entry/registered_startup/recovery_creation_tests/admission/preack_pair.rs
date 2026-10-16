// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Revalidate genuinely prepared pins and preheld creation before either peer can exist.
use super::*;
use crate::lifecycle_pipe::tests::prepared_direct_server as prepared_direct;
use crate::lifecycle_pipe::{
    self, LifecyclePipeBootstrap, LifecyclePipeError, LifecyclePipeSession, PeerBinding, PeerInput,
    current_admission::prepared_recovery_pair, enrollment::Enrollment,
};
use std::{error::Error, num::NonZeroUsize};
use sugarsubstitute_contracts::lifecycle::{ProcessId, ProcessKey};

fn held(
    fixture: &Fixture,
) -> TestResult<(crate::process_creation::RecoveryCreationPermit, Arc<Effects>)> {
    let effects = Arc::new(Effects::default());
    let mut input = Some(Input(effects.clone()));
    let permit = fixture.rig.client.reserve_recovery_custody_creation(
        &fixture.rig.scope,
        spec_for(7, &fixture.grant)?,
        &fixture.parent,
        &fixture.grant,
        &mut input,
        |input, _, _| Ok(Box::new(Driver(input))),
    )?;
    assert!(input.is_none());
    assert_eq!(effects.initialized.load(Ordering::Acquire), 0);
    Ok((permit, effects))
}
fn pins(fixture: &Fixture) -> TestResult<(LifecyclePipeBootstrap, Enrollment)> {
    Ok(Enrollment::take_bootstrap(&fixture.recovery.control.inner)?
        .ok_or("Actual prepared recovery pin missing")?)
}
fn retire_direct(rig: &Rig, mut session: LifecyclePipeSession) -> TestResult {
    session.control.close();
    drop(rig.completion(&mut session.completion)?);
    Ok(())
}
#[test]
fn preack_prepared_pair_allows_real_held_creation_without_peer_and_rejects_either_closed_source()
-> TestResult {
    for close_recovery in [false, true] {
        let fixture = fixture()?;
        let (scope, direct, pin, enrollment) = prepared_direct(&fixture.rig, &fixture.grant, 5, 6)?;
        let recovery_pins = pins(&fixture)?;
        let (permit, effects) = held(&fixture)?;
        prepared_recovery_pair(
            &fixture.recovery.control.inner,
            &fixture.grant,
            &direct.control.inner,
        )?;
        for current in [
            fixture.recovery.control.validate_current_admission(),
            direct.control.validate_current_admission(),
        ] {
            assert!(matches!(
                current.err().and_then(|cause| cause
                    .source()
                    .and_then(|cause| cause.downcast_ref::<LifecyclePipeError>())
                    .copied()),
                Some(LifecyclePipeError::NotReady),
            ));
        }
        assert_eq!(effects.initialized.load(Ordering::Acquire), 0);
        if close_recovery {
            fixture.recovery.control.close();
        } else {
            direct.control.close();
        }
        assert!(
            prepared_recovery_pair(
                &fixture.recovery.control.inner,
                &fixture.grant,
                &direct.control.inner
            )
            .is_err()
        );
        let mut job = permit.cancel_and_observe();
        let completion = fixture.rig.completion(&mut job)?;
        assert!(matches!(completion.outcome, JobOutcome::Cancelled));
        assert_eq!(effects.initialized.load(Ordering::Acquire), 0);
        assert_eq!(effects.dropped.load(Ordering::Acquire), 1);
        drop((pin, enrollment, recovery_pins));
        retire_direct(&fixture.rig, direct)?;
        drop((scope, job));
        finish(fixture, completion)?;
    }
    Ok(())
}
#[test]
fn preack_prepared_pair_rejects_equal_run_foreign_core_and_original_deadline_expiry() -> TestResult
{
    let fixture = fixture()?;
    let (scope, direct, pin, enrollment) = prepared_direct(&fixture.rig, &fixture.grant, 5, 6)?;
    let recovery_pins = pins(&fixture)?;
    let (permit, effects) = held(&fixture)?;
    let foreign = super::fixture()?;
    assert_eq!(foreign.grant.key(), fixture.grant.key());
    assert!(!foreign.grant.same_grant(&fixture.grant));
    let (foreign_scope, foreign_direct, foreign_pin, foreign_enrollment) =
        prepared_direct(&foreign.rig, &foreign.grant, 5, 6)?;
    assert!(
        prepared_recovery_pair(
            &fixture.recovery.control.inner,
            &foreign.grant,
            &direct.control.inner
        )
        .is_err()
    );
    assert!(
        prepared_recovery_pair(
            &fixture.recovery.control.inner,
            &fixture.grant,
            &foreign_direct.control.inner
        )
        .is_err()
    );
    let (foreign_permit, _) = held(&foreign)?;
    let mut foreign_job = foreign_permit.cancel_and_observe();
    let foreign_completion = foreign.rig.completion(&mut foreign_job)?;
    drop((foreign_pin, foreign_enrollment));
    retire_direct(&foreign.rig, foreign_direct)?;
    drop((foreign_scope, foreign_job));
    finish(foreign, foreign_completion)?;
    prepared_recovery_pair(&fixture.recovery.control.inner, &fixture.grant, &direct.control.inner)?;
    fixture.rig.clock.advance(fixture.grant.original_budget().until().as_nanos());
    assert!(
        prepared_recovery_pair(
            &fixture.recovery.control.inner,
            &fixture.grant,
            &direct.control.inner
        )
        .is_err()
    );
    let mut job = permit.cancel_and_observe();
    let completion = fixture.rig.completion(&mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::Cancelled | JobOutcome::TimedOut));
    assert_eq!(effects.initialized.load(Ordering::Acquire), 0);
    drop((pin, enrollment, recovery_pins));
    retire_direct(&fixture.rig, direct)?;
    drop((scope, job));
    finish(fixture, completion)
}
#[test]
fn preack_prepared_pair_rejects_real_enrollment_and_keeps_authenticated_current_stage_distinct()
-> TestResult {
    let fixture = fixture()?;
    let (scope, direct, pin, mut enrollment) = prepared_direct(&fixture.rig, &fixture.grant, 5, 6)?;
    let recovery_pins = pins(&fixture)?;
    let (permit, effects) = held(&fixture)?;
    prepared_recovery_pair(&fixture.recovery.control.inner, &fixture.grant, &direct.control.inner)?;
    let run = fixture.grant.key().run();
    enrollment
        .bind(
            PeerBinding::Desktop(run),
            ProcessKey { owner: run.owner, id: ProcessId::new(9)? },
            (),
            |_| PeerInput::Simulated,
        )
        .map_err(|(cause, ())| cause)?;
    assert!(
        prepared_recovery_pair(
            &fixture.recovery.control.inner,
            &fixture.grant,
            &direct.control.inner
        )
        .is_err()
    );
    loop {
        let revision = fixture.rig.wake.count();
        if matches!(
            direct.control.take_observation(),
            Some(lifecycle_pipe::LifecyclePipeObservation::Connected)
        ) {
            break;
        }
        fixture.rig.client.drain_completions(NonZeroUsize::MIN)?;
        if matches!(
            direct.control.take_observation(),
            Some(lifecycle_pipe::LifecyclePipeObservation::Connected)
        ) {
            break;
        }
        fixture.rig.wake.wait_after(revision)?;
    }
    direct.control.verify_capability([7; 32], fixture.grant.original_budget().until())?;
    loop {
        let revision = fixture.rig.wake.count();
        if direct.control.validate_current_admission().is_ok() {
            break;
        }
        fixture.rig.client.drain_completions(NonZeroUsize::MIN)?;
        if direct.control.validate_current_admission().is_ok() {
            break;
        }
        fixture.rig.wake.wait_after(revision)?;
    }
    assert!(
        prepared_recovery_pair(
            &fixture.recovery.control.inner,
            &fixture.grant,
            &direct.control.inner
        )
        .is_err()
    );
    let mut job = permit.cancel_and_observe();
    let completion = fixture.rig.completion(&mut job)?;
    assert_eq!(effects.initialized.load(Ordering::Acquire), 0);
    drop((pin, enrollment, recovery_pins));
    retire_direct(&fixture.rig, direct)?;
    drop((scope, job));
    finish(fixture, completion)
}
