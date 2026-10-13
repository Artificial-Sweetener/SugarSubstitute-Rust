// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real output artifacts exercise the public capsule API; canonical FS is boundary-faked.
//! This native pipe fixture is compiled in the author lane and executed by the root.
use super::*;
use sugarsubstitute_execution::windows_output::{MIN_WINDOWS_OUTPUT_BYTES, WindowsOutputSession};

fn prepared(
    h: &mut Harness,
    resource: ResourceKey,
) -> Result<(WindowsOutputSession, Scope), Box<dyn Error>> {
    let scope = h.client.create_native_scope(
        ScopeSpec { id: ScopeId::new(resource.id.get() + 2)?, deadline: None },
        None,
    )?;
    let mut spec = h.spec(MIN_WINDOWS_OUTPUT_BYTES)?;
    spec.class = JobClass::NativeObservation;
    let mut session = h.client.reserve_windows_output(
        &scope,
        spec,
        sugarsubstitute_execution::windows_output::OutputIdentity::Managed(resource),
    )?;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let revision = *lock(&h.wake.revision);
        h.client.drain_completions(std::num::NonZeroUsize::MIN)?;
        if session.control.snapshot().prepared {
            return Ok((session, scope));
        }
        if let Some(completion) = session.completion.try_take_completion() {
            return Err(format!("Output preparation ended: {:?}", completion.outcome).into());
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("Output preparation deadline expired".into());
        }
        let (_guard, timeout) = h
            .wake
            .changed
            .wait_timeout_while(lock(&h.wake.revision), remaining, |current| *current == revision)
            .map_err(|_| "Output wake poisoned")?;
        if timeout.timed_out() {
            return Err("Output preparation wake deadline expired".into());
        }
    }
}

#[test]
fn ffi_actual_managed_output_rejection_preserves_capsule_and_accepts_exact_session()
-> Result<(), Box<dyn Error>> {
    let mut h = Harness::new()?;
    h.initialize()?;
    let resolution = h.resolve()?;
    let resource = h.resource(1)?;
    let spec = h.spec(CLAIM_OPERATION_BYTES)?;
    h.claims.submit_backend_claim(&h.client, &h.scope, spec, resource, &resolution)?;
    h.drain()?;
    let capsule = h.claims.prepare_managed_launch(
        resource,
        crate::LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?,
    )?;
    let origin = capsule.origin.clone();
    let proof = capsule.proof.clone();
    let preparation = capsule.prepared.clone();
    let required = capsule.required_reservation_bytes();
    let (mut matching, matching_scope) = prepared(&mut h, resource)?;
    let foreign_resource = h.resource(2)?;
    let (mut foreign, foreign_scope) = prepared(&mut h, foreign_resource)?;
    let stdio = matching.control.take_child_stdio()?;
    let rejected = capsule
        .with_owned_output(&foreign.control, stdio)
        .err()
        .ok_or("Foreign control accepted managed output")?;
    assert!(matches!(rejected.error, crate::RegistryInputError::OutputProvenance));
    assert!(Arc::ptr_eq(&origin, &rejected.input.origin));
    assert!(Arc::ptr_eq(&proof, &rejected.input.proof));
    assert!(Arc::ptr_eq(&preparation, &rejected.input.prepared));
    assert_eq!(preparation.load(Ordering::Acquire), 1);
    assert_eq!(rejected.input.required_reservation_bytes(), required);
    rejected.stdio.borrow_inheritance(
        sugarsubstitute_execution::windows_output::OutputIdentity::Managed(resource),
    )?;
    assert!(!matching.control.snapshot().retired);
    assert!(!foreign.control.snapshot().retired);
    let capsule = (*rejected.input).with_owned_output(&matching.control, rejected.stdio)?;
    assert!(Arc::ptr_eq(&origin, &capsule.origin));
    assert!(Arc::ptr_eq(&proof, &capsule.proof));
    assert_eq!(preparation.load(Ordering::Acquire), 1);
    let foreign_stdio = foreign.control.take_child_stdio()?;
    let rejected = capsule
        .with_owned_output(&foreign.control, foreign_stdio)
        .err()
        .ok_or("Second output replaced the original session")?;
    assert!(matches!(rejected.error, crate::RegistryInputError::OutputConflict));
    rejected.stdio.borrow_inheritance(
        sugarsubstitute_execution::windows_output::OutputIdentity::Managed(foreign_resource),
    )?;
    assert_eq!(preparation.load(Ordering::Acquire), 1);
    drop(rejected);
    assert_eq!(preparation.load(Ordering::Acquire), 2);
    foreign.control.close();
    matching.control.close();
    let matched_terminal = h.completion(&mut matching.completion)?;
    let foreign_terminal = h.completion(&mut foreign.completion)?;
    assert!(matches!(
        matched_terminal.outcome,
        sugarsubstitute_contracts::execution::JobOutcome::Cancelled
    ));
    assert!(matches!(
        foreign_terminal.outcome,
        sugarsubstitute_contracts::execution::JobOutcome::Cancelled
    ));
    assert!(matching.control.snapshot().retired);
    assert!(foreign.control.snapshot().retired);
    drop((
        matched_terminal,
        foreign_terminal,
        matching,
        foreign,
        matching_scope,
        foreign_scope,
        proof,
    ));
    let spec = h.spec(CLAIM_OPERATION_BYTES)?;
    h.claims.submit_backend_release(&h.client, &h.scope, spec, &h.registry, resource)?;
    h.drain()?;
    h.close(0)?;
    Ok(())
}
