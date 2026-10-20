// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual retained initialization guard through finite execution jobs.
use super::*;
use crate::{
    claims::entry::fixture::PublicationFixture,
    coordinator_entry::election::{ElectionRecordOrigin, FreshRecordOrigin},
    protected_storage::record_file::RecordRead,
};
use sugarsubstitute_contracts::{execution::JobContext, lifecycle::NamespaceId};

fn check_proof(
    fixture: &mut PublicationFixture,
    resolve: impl FnOnce(&JobContext) -> crate::protected_storage::account::ResolvedNamespace
    + Send
    + 'static,
    deadline: Moment,
) -> Result<bool, Box<dyn Error>> {
    let proof = fixture.retained_proof()?;
    let spec = fixture.entry.spec(deadline)?;
    let mut job =
        fixture.entry.client.submit_blocking(&fixture.entry.scope, spec, move |context| {
            let resolved = resolve(&context);
            match proof.lock_for_initialization(&context, &resolved) {
                Ok(guard) => {
                    guard.check_record(&RecordRead::Missing)?;
                    Ok::<_, ClaimError>(true)
                }
                Err(_) => Ok(false),
            }
        })?;
    let done = fixture.entry.consume_job(&mut job)?;
    match done.outcome {
        JobOutcome::Success(value) => Ok(value),
        other => Err(format!("guard produced unexpected outcome: {other:?}").into()),
    }
}

#[test]
fn bound_guard_checks_actual_account_installation_namespace_and_original_deadline() -> TestResult {
    let mut fixture =
        PublicationFixture::new(ElectionRecordOrigin::Fresh(FreshRecordOrigin::Missing))?;
    let until = fixture.entry.budget.until();
    assert!(check_proof(&mut fixture, |_| resolved(), until)?);
    assert!(!check_proof(
        &mut fixture,
        |_| {
            let mut value = resolved();
            value.namespace = NamespaceId::new([2; 32]);
            value
        },
        until
    )?);
    assert!(!check_proof(
        &mut fixture,
        |_| {
            let mut value = resolved();
            value.installation_identity = [2; 24];
            value
        },
        until
    )?);
    assert!(!check_proof(
        &mut fixture,
        |_| {
            let mut value = resolved();
            value.account_sid[8] = 43;
            value
        },
        until
    )?);
    assert!(!check_proof(&mut fixture, |_| resolved(), Moment::from_nanos(until.as_nanos() + 1))?);
    fixture.entry.close(Moment::from_nanos(40_000_000_000))?;
    Ok(())
}

#[test]
fn retained_guard_rejects_foreign_actual_execution_and_changed_external_election() -> TestResult {
    let mut fixture =
        PublicationFixture::new(ElectionRecordOrigin::Fresh(FreshRecordOrigin::Missing))?;
    let other = ExecutionOwner::new(fixture.entry.clock.clone(), fixture.entry.wake.clone())?;
    let client = other.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let proof = fixture.retained_proof()?;
    let spec = fixture.entry.setup_spec()?;
    let mut job = client.submit_blocking(&scope, spec, move |context| {
        Ok::<_, ClaimError>(proof.lock_for_initialization(&context, &resolved()).is_err())
    })?;
    let done = fixture.entry.consume_job(&mut job)?;
    assert!(matches!(done.outcome, JobOutcome::Success(true)));
    fixture.entry.fake.locked.store(false, Ordering::Release);
    let until = fixture.entry.budget.until();
    assert!(!check_proof(&mut fixture, |_| resolved(), until)?);
    fixture.entry.fake.locked.store(true, Ordering::Release);
    lock(&fixture.entry.owner.lifetime.state).record_origin =
        Some(ElectionRecordOrigin::Fresh(FreshRecordOrigin::Empty));
    assert!(!check_proof(&mut fixture, |_| resolved(), until)?);
    fixture.entry.close(Moment::from_nanos(40_000_000_000))?;
    Ok(())
}

#[test]
fn claimed_election_guard_preserves_borrow_and_closes_atomically_before_retirement() -> TestResult {
    for close_claims in [false, true] {
        let mut fixture =
            PublicationFixture::new(ElectionRecordOrigin::Fresh(FreshRecordOrigin::Missing))?;
        let proof = fixture.retained_proof()?;
        let gate = Arc::new(Gate::default());
        let worker_gate = gate.clone();
        let spec = fixture.entry.setup_spec()?;
        let mut job =
            fixture.entry.client.submit_blocking(&fixture.entry.scope, spec, move |context| {
                let guard = proof.lock_for_initialization(&context, &resolved())?;
                guard.check_record(&RecordRead::Missing)?;
                worker_gate.enter()?;
                Ok::<_, ClaimError>(guard.validate_retained().is_err())
            })?;
        gate.wait()?;
        assert!(matches!(
            fixture.entry.owner.lifetime.state.try_lock(),
            Err(std::sync::TryLockError::WouldBlock)
        ));
        if close_claims {
            fixture.close_claim_admission();
        } else {
            fixture.entry.owner.close_admission();
        }
        assert!(!fixture.entry.owner.lifetime.retired.load(Ordering::Acquire));
        gate.release();
        let done = fixture.entry.consume_job(&mut job)?;
        assert!(matches!(done.outcome, JobOutcome::Success(true)));
        fixture.entry.close(Moment::from_nanos(40_000_000_000))?;
        assert!(fixture.entry.owner.lifetime.retired.load(Ordering::Acquire));
    }
    Ok(())
}

#[test]
fn expired_original_setup_rejects_before_guard_can_grant_initialization() -> TestResult {
    let mut fixture =
        PublicationFixture::new(ElectionRecordOrigin::Fresh(FreshRecordOrigin::Missing))?;
    let clock = fixture.entry.clock.clone();
    let original_until = fixture.entry.budget.until();
    let guard_until = original_until.checked_add(std::time::Duration::from_secs(10))?;
    assert!(!check_proof(
        &mut fixture,
        move |_| {
            clock.0.store(original_until.as_nanos(), Ordering::Release);
            resolved()
        },
        guard_until
    )?);
    fixture.entry.close(original_until.checked_add(std::time::Duration::from_secs(20))?)?;
    Ok(())
}
