// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise repeated main attempts through the real finite execution/profile boundary.
use super::support::{Harness, TestResult, main};
use super::*;
use sugarsubstitute_contracts::startup::MainIdentity;

fn folder(
    h: &mut Harness,
    identity: MainIdentity,
) -> Result<ProfileFolderFacts, Box<dyn std::error::Error>> {
    let spec = h.spec(100)?;
    h.lease.submit_folder_preparation(&h.client, &h.scope, spec, identity)?;
    assert_eq!(h.completion()?.kind, TerminalKind::Success);
    h.lease.take_folder_facts().ok_or_else(|| "missing fresh folder".into())
}
fn environment(
    h: &mut Harness,
    identity: MainIdentity,
) -> Result<EnvironmentProfileProof, Box<dyn std::error::Error>> {
    let spec = h.spec(100)?;
    h.lease.submit_environment_validation(
        &h.client,
        &h.scope,
        spec,
        identity,
        &[69, 58, 92, 112],
    )?;
    assert_eq!(h.completion()?.kind, TerminalKind::Success);
    h.lease.take_environment_proof().ok_or_else(|| "missing fresh environment proof".into())
}

#[test]
fn repeated_main_attempts_prepare_fresh_folders_and_environment_proofs() -> TestResult {
    let mut h = Harness::new()?;
    h.acquire()?;
    let first = folder(&mut h, main(1)?)?;
    let first_environment = environment(&mut h, main(1)?)?;
    assert!(first.validates(&h.lease, main(1)?));
    assert!(first_environment.validates(&h.lease, main(1)?));
    h.lease.retire_main_attempt(main(1)?)?;
    assert!(!first.validates(&h.lease, main(1)?));
    assert!(!first_environment.validates(&h.lease, main(1)?));
    h.lease.confirm_main_retired(main(1)?)?;
    h.lease.begin_main_attempt(main(2)?)?;
    let second = folder(&mut h, main(2)?)?;
    let second_environment = environment(&mut h, main(2)?)?;
    assert!(second.validates(&h.lease, main(2)?));
    assert!(second_environment.validates(&h.lease, main(2)?));
    assert_eq!(first.units(), second.units());
    assert_ne!(first.units().as_ptr(), second.units().as_ptr());
    assert!(!first.validates(&h.lease, main(2)?));
    assert!(!first_environment.validates(&h.lease, main(2)?));
    h.lease.retire_main_attempt(main(2)?)?;
    h.lease.confirm_main_retired(main(2)?)?;
    h.lease.begin_main_attempt(main(3)?)?;
    assert!(!first.validates(&h.lease, main(1)?));
    assert!(!second.validates(&h.lease, main(2)?));
    assert!(h.lease.snapshot().held);
    assert_eq!(h.fake.acquires.load(Ordering::Acquire), 1);
    assert_eq!(h.fake.releases.load(Ordering::Acquire), 0);
    Ok(())
}

#[test]
fn retired_observation_requires_closed_exact_attempt_and_consumed_current_job() -> TestResult {
    let mut h = Harness::new()?;
    h.acquire()?;
    assert!(h.lease.confirm_main_retired(main(1)?).is_err());
    assert!(h.lease.begin_main_attempt(main(2)?).is_err());
    let spec = h.spec(100)?;
    h.lease.submit_folder_preparation(&h.client, &h.scope, spec, main(1)?)?;
    h.lease.retire_main_attempt(main(1)?)?;
    assert!(h.lease.confirm_main_retired(main(1)?).is_err());
    assert!(h.lease.begin_main_attempt(main(2)?).is_err());
    h.completion()?;
    assert!(h.lease.take_folder_facts().is_none());
    assert!(h.lease.confirm_main_retired(main(2)?).is_err());
    h.lease.confirm_main_retired(main(1)?)?;
    h.lease.confirm_main_retired(main(1)?)?;
    assert!(h.lease.begin_main_attempt(main(1)?).is_err());
    let mut foreign = main(2)?;
    foreign.run.generation = sugarsubstitute_contracts::lifecycle::RunGeneration::new(2)?;
    assert!(h.lease.begin_main_attempt(foreign).is_err());
    h.lease.begin_main_attempt(main(2)?)?;
    assert!(h.lease.retire_main_attempt(main(1)?).is_err());
    assert!(folder(&mut h, main(2)?)?.validates(&h.lease, main(2)?));
    Ok(())
}

#[test]
fn rejected_release_never_reopens_proofs_and_keeps_original_deadline_for_retry() -> TestResult {
    let mut h = Harness::new()?;
    h.acquire()?;
    let facts = folder(&mut h, main(1)?)?;
    let proof = environment(&mut h, main(1)?)?;
    let mut rejected = h.spec(100)?;
    rejected.reservation_bytes = u64::MAX;
    assert!(matches!(
        h.lease.submit_release(&h.client, &h.scope, rejected, &h.registry),
        Err(ClaimAdmissionError::Execution(_))
    ));
    assert!(!facts.validates(&h.lease, main(1)?));
    assert!(!proof.validates(&h.lease, main(1)?));
    assert!(h.lease.snapshot().held);
    assert!(!h.lease.snapshot().pending);
    assert_eq!(h.fake.releases.load(Ordering::Acquire), 0);
    let renewed = h.spec(200)?;
    assert!(h.lease.submit_release(&h.client, &h.scope, renewed, &h.registry).is_err());
    assert!(h.lease.begin_main_attempt(main(2)?).is_err());
    assert_eq!(h.release(100)?.kind, TerminalKind::Success);
    assert!(!facts.validates(&h.lease, main(1)?));
    assert!(!proof.validates(&h.lease, main(1)?));
    assert!(h.lease.snapshot().released);
    Ok(())
}

#[test]
fn uncertain_browser_cannot_be_reset_by_matching_retirement_or_new_generation() -> TestResult {
    let mut h = Harness::new()?;
    h.acquire()?;
    let spec = h.spec(100)?;
    assert!(
        h.lease
            .submit_browser_validation(&h.client, &h.scope, spec, &h.registry, main(1)?, 0)
            .is_err()
    );
    h.lease.retire_main_attempt(main(1)?)?;
    assert!(h.lease.confirm_main_retired(main(1)?).is_err());
    assert!(h.lease.begin_main_attempt(main(2)?).is_err());
    assert!(h.lease.snapshot().uncertain);
    assert_eq!(h.fake.releases.load(Ordering::Acquire), 0);
    Ok(())
}

#[test]
fn finite_folder_copy_cannot_renew_attempt_budget_and_retains_distinct_memory() -> TestResult {
    let mut h = Harness::new()?;
    h.acquire()?;
    let first = folder(&mut h, main(1)?)?;
    let renewed = h.spec(200)?;
    assert!(
        h.lease
            .submit_environment_validation(
                &h.client,
                &h.scope,
                renewed,
                main(1)?,
                &[69, 58, 92, 112]
            )
            .is_err()
    );
    h.lease.retire_main_attempt(main(1)?)?;
    h.lease.confirm_main_retired(main(1)?)?;
    h.lease.begin_main_attempt(main(2)?)?;
    let second = folder(&mut h, main(2)?)?;
    assert!(h.scope.snapshot().reserved_bytes >= 3 * PROFILE_OPERATION_BYTES);
    h.release(100)?;
    h.finish_workers();
    drop(h.lease);
    assert_eq!(h.scope.snapshot().reserved_bytes, 3 * PROFILE_OPERATION_BYTES);
    drop(first);
    assert_eq!(h.scope.snapshot().reserved_bytes, 2 * PROFILE_OPERATION_BYTES);
    drop(second);
    assert_eq!(h.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn retained_old_folder_rejects_new_admission_until_its_actual_charge_is_released() -> TestResult {
    let mut h = Harness::new()?;
    h.acquire()?;
    let mut spec = h.spec(100)?;
    let table_bytes = h.table_reserved_bytes();
    spec.reservation_bytes = (254 * 1024 * 1024_u64)
        .checked_sub(table_bytes)
        .ok_or("Actual table exceeded capacity fixture")?;
    h.lease.submit_folder_preparation(&h.client, &h.scope, spec, main(1)?)?;
    assert_eq!(h.completion()?.kind, TerminalKind::Success);
    let retained = h.lease.take_folder_facts().ok_or("missing retained folder")?;
    h.lease.retire_main_attempt(main(1)?)?;
    h.lease.confirm_main_retired(main(1)?)?;
    h.lease.begin_main_attempt(main(2)?)?;
    let spec = h.spec(100)?;
    assert!(matches!(
        h.lease.submit_folder_preparation(&h.client, &h.scope, spec, main(2)?),
        Err(ClaimAdmissionError::Execution(
            sugarsubstitute_contracts::execution::AdmissionError::MemorySaturated { .. }
        ))
    ));
    assert_eq!(h.scope.snapshot().reserved_bytes + table_bytes, 256 * 1024 * 1024);
    assert!(!retained.validates(&h.lease, main(1)?));
    drop(retained);
    h.lease.retire_main_attempt(main(2)?)?;
    h.lease.confirm_main_retired(main(2)?)?;
    assert_eq!(h.release_on_activity(100)?.kind, TerminalKind::Success);
    h.finish_workers();
    assert_eq!(h.scope.snapshot().reserved_bytes, PROFILE_OPERATION_BYTES);
    Ok(())
}

#[test]
fn browser_submission_slots_advance_only_after_exact_retirement_without_reviving_old_binding()
-> TestResult {
    let mut attempts = attempt::Attempts::new();
    attempts.begin(main(1)?)?;
    let deadline = sugarsubstitute_contracts::execution::Moment::from_nanos(100);
    let first = attempts.admit(main(1)?, deadline, attempt::ProofKind::Browser)?;
    assert!(first.live(main(1)?));
    assert!(attempts.admit(main(1)?, deadline, attempt::ProofKind::Browser).is_err());
    attempts.retire(main(1)?)?;
    assert!(!first.live(main(1)?));
    assert!(attempts.begin(main(2)?).is_err());
    attempts.confirm(main(1)?)?;
    let mut unchanged = main(2)?;
    unchanged.launch = main(1)?.launch;
    assert!(attempts.begin(unchanged).is_err());
    let mut retry = main(2)?;
    retry.navigation = main(1)?.navigation;
    attempts.begin(retry)?;
    let second = attempts.admit(retry, deadline, attempt::ProofKind::Browser)?;
    assert!(second.live(retry));
    assert!(!first.live(main(1)?));
    assert!(!first.live(main(2)?));
    assert!(attempts.retire(main(1)?).is_err());
    assert!(second.live(retry));
    Ok(())
}

#[test]
fn retry_launch_resets_navigation_without_reviving_prior_lease_proofs() -> TestResult {
    let mut h = Harness::new()?;
    h.acquire()?;
    let first = folder(&mut h, main(1)?)?;
    let first_environment = environment(&mut h, main(1)?)?;
    h.lease.retire_main_attempt(main(1)?)?;
    h.lease.confirm_main_retired(main(1)?)?;
    let mut retry = main(2)?;
    retry.navigation = main(1)?.navigation;
    h.lease.begin_main_attempt(retry)?;
    let fresh = folder(&mut h, retry)?;
    let fresh_environment = environment(&mut h, retry)?;
    assert!(fresh.validates(&h.lease, retry));
    assert!(fresh_environment.validates(&h.lease, retry));
    assert!(!first.validates(&h.lease, main(1)?));
    assert!(!first.validates(&h.lease, retry));
    assert!(!first_environment.validates(&h.lease, main(1)?));
    assert!(!first_environment.validates(&h.lease, retry));
    assert!(!fresh.validates(&h.lease, main(1)?));
    assert!(!fresh_environment.validates(&h.lease, main(1)?));
    let spec = h.spec(100)?;
    assert!(matches!(
        h.lease.submit_folder_preparation(&h.client, &h.scope, spec, main(1)?),
        Err(ClaimAdmissionError::Identity)
    ));
    assert_eq!(h.fake.acquires.load(Ordering::Acquire), 1);
    assert_eq!(h.fake.releases.load(Ordering::Acquire), 0);
    assert!(h.lease.snapshot().held);
    Ok(())
}

#[test]
fn reset_navigation_retry_requires_exact_retirement_and_strictly_newer_same_run_launch()
-> TestResult {
    let mut h = Harness::new()?;
    h.acquire()?;
    let mut retry = main(2)?;
    retry.navigation = main(1)?.navigation;
    assert!(matches!(h.lease.begin_main_attempt(retry), Err(ClaimAdmissionError::Busy)));
    h.lease.retire_main_attempt(main(1)?)?;
    assert!(matches!(h.lease.begin_main_attempt(retry), Err(ClaimAdmissionError::Busy)));
    h.lease.confirm_main_retired(main(1)?)?;
    let mut same_launch = main(2)?;
    same_launch.launch = main(1)?.launch;
    assert!(matches!(h.lease.begin_main_attempt(same_launch), Err(ClaimAdmissionError::Identity)));
    let mut foreign = retry;
    foreign.run.generation = sugarsubstitute_contracts::lifecycle::RunGeneration::new(2)?;
    assert!(matches!(h.lease.begin_main_attempt(foreign), Err(ClaimAdmissionError::Identity)));
    h.lease.begin_main_attempt(retry)?;
    h.lease.retire_main_attempt(retry)?;
    h.lease.confirm_main_retired(retry)?;
    assert!(matches!(h.lease.begin_main_attempt(main(1)?), Err(ClaimAdmissionError::Identity)));
    assert!(matches!(h.lease.begin_main_attempt(retry), Err(ClaimAdmissionError::Identity)));
    let mut successor = main(3)?;
    successor.navigation = main(1)?.navigation;
    h.lease.begin_main_attempt(successor)?;
    assert!(folder(&mut h, successor)?.validates(&h.lease, successor));
    Ok(())
}
