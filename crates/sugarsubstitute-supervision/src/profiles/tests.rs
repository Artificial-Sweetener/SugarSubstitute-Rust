// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Characterize profile acquisition, proof and release through the actual owner.
mod acknowledgement_retirement;
#[path = "attempt_tests.rs"]
mod attempt_tests;
mod release_observation;
mod release_progress;
mod support;
use super::*;
use sugarsubstitute_contracts::{
    execution::{ScopeId, ScopeSpec},
    lifecycle::CoordinatorEpoch,
};
use support::{Harness, TestResult, input, main, run};

#[test]
fn lexical_construction_has_no_effect_and_rejects_foreign_or_nonlocal_paths() -> TestResult {
    let harness = Harness::new()?;
    assert_eq!(harness.fake.acquires.load(Ordering::Acquire), 0);
    assert!(!harness.lease.snapshot().held);
    assert!(
        WindowsProfileLease::new(run()?, PathBuf::from("relative"), &harness.registry).is_err()
    );
    let mut foreign = run()?;
    foreign.owner.epoch = CoordinatorEpoch::new([9; 16])?;
    assert!(
        WindowsProfileLease::new(foreign, PathBuf::from(r"E:\fixture"), &harness.registry).is_err()
    );
    Ok(())
}
#[test]
fn environment_proof_is_single_use_and_invalidated_before_release() -> TestResult {
    let mut h = Harness::new()?;
    assert_eq!(h.acquire()?.kind, TerminalKind::Success);
    let spec = h.spec(100)?;
    h.lease.submit_environment_validation(
        &h.client,
        &h.scope,
        spec,
        main(1)?,
        &[69, 58, 92, 112],
    )?;
    assert_eq!(h.completion()?.kind, TerminalKind::Success);
    let proof = h.lease.take_environment_proof().ok_or("Missing proof")?;
    assert!(proof.validates(&h.lease, main(1)?));
    assert!(h.lease.take_environment_proof().is_none());
    let spec = h.spec(100)?;
    assert!(
        h.lease
            .submit_environment_validation(&h.client, &h.scope, spec, main(1)?, &[69, 58, 92, 112])
            .is_err()
    );
    let spec = h.spec(100)?;
    h.lease.submit_release(&h.client, &h.scope, spec, &h.registry)?;
    assert!(!proof.validates(&h.lease, main(1)?));
    assert_eq!(h.completion()?.kind, TerminalKind::Success);
    assert!(h.lease.snapshot().released);
    Ok(())
}
#[test]
fn partial_failed_acquisition_retains_exclusion_until_checked_cleanup() -> TestResult {
    let mut h = Harness::new()?;
    h.fake.fail_acquire.store(true, Ordering::Release);
    let result = h.acquire()?;
    assert!(result.failure.is_some());
    assert_eq!(result.kind, TerminalKind::Failure);
    assert!(h.lease.snapshot().held);
    assert!(!h.lease.snapshot().released);
    assert!(h.lease.prepare_desktop_launch(input()?).is_err());
    assert_eq!(h.release(200)?.kind, TerminalKind::Success);
    assert_eq!(h.fake.releases.load(Ordering::Acquire), 1);
    assert!(h.lease.snapshot().released);
    Ok(())
}
#[test]
fn late_discarded_success_keeps_ownership_without_authorizing_launch() -> TestResult {
    let mut h = Harness::new()?;
    *lock(&h.fake.late_acquire) = Some(h.clock.clone());
    assert_eq!(h.acquire()?.kind, TerminalKind::TimedOut);
    assert!(h.lease.snapshot().held);
    assert!(h.lease.prepare_desktop_launch(input()?).is_err());
    assert_eq!(h.release(200)?.kind, TerminalKind::Success);
    assert!(h.lease.snapshot().released);
    Ok(())
}
#[test]
fn mismatched_environment_produces_no_proof_and_remains_releasable_without_launch() -> TestResult {
    let mut h = Harness::new()?;
    h.acquire()?;
    let spec = h.spec(100)?;
    h.lease.submit_environment_validation(
        &h.client,
        &h.scope,
        spec,
        main(1)?,
        &[69, 58, 92, 120],
    )?;
    assert!(h.completion()?.failure.is_some());
    assert!(h.lease.take_environment_proof().is_none());
    assert_eq!(h.release(100)?.kind, TerminalKind::Success);
    Ok(())
}
#[test]
fn failed_close_preserves_state_and_rejects_renewed_cleanup_budget() -> TestResult {
    let mut h = Harness::new()?;
    h.acquire()?;
    h.fake.fail_release.store(true, Ordering::Release);
    assert!(h.release(100)?.failure.is_some());
    assert!(h.lease.snapshot().held);
    assert!(!h.lease.snapshot().released);
    h.fake.fail_release.store(false, Ordering::Release);
    assert!(h.release(200).is_err());
    assert_eq!(h.fake.releases.load(Ordering::Acquire), 1);
    assert_eq!(h.release(100)?.kind, TerminalKind::Success);
    assert_eq!(h.fake.releases.load(Ordering::Acquire), 2);
    Ok(())
}
#[test]
fn outstanding_capsule_blocks_release_until_its_exact_drop() -> TestResult {
    let mut h = Harness::new()?;
    h.acquire()?;
    let capsule = h.lease.prepare_desktop_launch(input()?)?;
    let spec = h.spec(100)?;
    assert!(h.lease.submit_release(&h.client, &h.scope, spec, &h.registry).is_err());
    drop(capsule);
    assert_eq!(h.release(100)?.kind, TerminalKind::Success);
    Ok(())
}
#[test]
fn foreign_registry_cannot_release_matching_public_run_facts() -> TestResult {
    let mut h = Harness::new()?;
    h.acquire()?;
    let foreign = h.construct_registry(run()?.owner)?;
    let spec = h.spec(100)?;
    assert!(h.lease.submit_release(&h.client, &h.scope, spec, &foreign).is_err());
    assert_eq!(h.fake.releases.load(Ordering::Acquire), 0);
    assert_eq!(h.release(100)?.kind, TerminalKind::Success);
    Ok(())
}
#[test]
fn uncertain_browser_lifetime_blocks_release_without_a_retained_family() -> TestResult {
    let mut h = Harness::new()?;
    h.acquire()?;
    let browser_spec = h.spec(100)?;
    assert!(
        h.lease
            .submit_browser_validation(
                &h.client,
                &h.scope,
                browser_spec,
                &h.registry,
                main(1)?,
                123
            )
            .is_err()
    );
    let spec = h.spec(100)?;
    assert!(h.lease.submit_release(&h.client, &h.scope, spec, &h.registry).is_err());
    assert!(h.lease.snapshot().uncertain);
    assert_eq!(h.fake.releases.load(Ordering::Acquire), 0);
    Ok(())
}

#[test]
fn canceled_scope_cannot_authorize_even_when_native_work_never_started() -> TestResult {
    let mut h = Harness::new()?;
    h.scope.cancel();
    let spec = h.spec(100)?;
    match h.lease.submit_acquire(&h.client, &h.scope, spec) {
        Ok(_) => {
            let outcome = h.completion()?;
            assert_eq!(outcome.kind, TerminalKind::Cancelled);
            assert!(h.lease.take_folder_facts().is_none());
            assert!(h.lease.prepare_desktop_launch(input()?).is_err());
        }
        Err(ClaimAdmissionError::Execution(_)) => {}
        Err(cause) => return Err(cause.into()),
    }
    assert_eq!(h.fake.acquires.load(Ordering::Acquire), 0);
    assert!(!h.lease.snapshot().held);
    Ok(())
}
#[test]
fn canonical_folder_facts_are_owned_once_and_invalidated_with_retirement() -> TestResult {
    let mut h = Harness::new()?;
    h.acquire()?;
    h.folder()?;
    let facts = h.lease.take_folder_facts().ok_or("Missing canonical folder")?;
    assert_eq!(facts.units(), &[69, 58, 92, 112]);
    assert!(facts.validates(&h.lease, main(1)?));
    assert!(h.lease.take_folder_facts().is_none());
    h.release(100)?;
    assert!(!facts.validates(&h.lease, main(1)?));
    assert_eq!(facts.units(), &[69, 58, 92, 112]);
    Ok(())
}

#[test]
fn zero_browser_identity_is_uncertain_and_cannot_unlock_profile() -> TestResult {
    let mut h = Harness::new()?;
    h.acquire()?;
    let spec = h.spec(100)?;
    assert!(
        h.lease
            .submit_browser_validation(&h.client, &h.scope, spec, &h.registry, main(1)?, 0)
            .is_err()
    );
    assert!(h.lease.snapshot().uncertain);
    let spec = h.spec(100)?;
    assert!(h.lease.submit_release(&h.client, &h.scope, spec, &h.registry).is_err());
    assert_eq!(h.fake.releases.load(Ordering::Acquire), 0);
    Ok(())
}

#[test]
fn rejected_launch_without_native_record_can_release_its_exact_profile() -> TestResult {
    let mut h = Harness::new()?;
    h.acquire()?;
    let capsule = h.lease.prepare_desktop_launch(input()?)?;
    let creation = h.spec(100)?;
    let root_watch = h.spec(100)?;
    let jobs = crate::ClaimLaunchJobs {
        native_scope: ScopeSpec { id: ScopeId::new(3)?, deadline: None },
        creation,
        root_watch,
    };
    assert!(
        h.lease.submit_desktop_launch(&h.client, &mut h.registry, &h.scope, jobs, capsule).is_err()
    );
    assert!(h.registry.retained_desktop(run()?)?.is_none());
    assert_eq!(h.release(100)?.kind, TerminalKind::Success);
    Ok(())
}
#[test]
fn foreign_capsule_cannot_create_a_family_in_another_matching_run_owner() -> TestResult {
    let mut original = Harness::new()?;
    original.acquire()?;
    let capsule = original.lease.prepare_desktop_launch(input()?)?;
    let mut foreign = Harness::new()?;
    foreign.acquire()?;
    let creation = foreign.spec(100)?;
    let root_watch = foreign.spec(100)?;
    let jobs = crate::ClaimLaunchJobs {
        native_scope: ScopeSpec { id: ScopeId::new(3)?, deadline: None },
        creation,
        root_watch,
    };
    assert!(
        foreign
            .lease
            .submit_desktop_launch(
                &foreign.client,
                &mut foreign.registry,
                &foreign.scope,
                jobs,
                capsule
            )
            .is_err()
    );
    assert!(original.registry.retained_desktop(run()?)?.is_none());
    assert!(foreign.registry.retained_desktop(run()?)?.is_none());
    assert_eq!(original.release(100)?.kind, TerminalKind::Success);
    assert_eq!(foreign.release(100)?.kind, TerminalKind::Success);
    Ok(())
}
#[test]
fn absent_accepted_family_never_substitutes_for_observed_settlement() -> TestResult {
    let mut h = Harness::new()?;
    h.acquire()?;
    h.lease.launch = LaunchState::Accepted(crate::LaunchTicket {
        process: sugarsubstitute_contracts::lifecycle::ProcessKey {
            owner: run()?.owner,
            id: sugarsubstitute_contracts::lifecycle::ProcessId::new(1)?,
        },
        binding: crate::FamilyBinding::Desktop(run()?),
    });
    let spec = h.spec(100)?;
    assert!(h.lease.submit_release(&h.client, &h.scope, spec, &h.registry).is_err());
    assert_eq!(h.fake.releases.load(Ordering::Acquire), 0);
    Ok(())
}

#[test]
fn folder_storage_keeps_actual_admission_charge_until_its_last_owner_drops() -> TestResult {
    let mut h = Harness::new()?;
    h.acquire()?;
    h.folder()?;
    let facts = h.lease.take_folder_facts().ok_or("Missing folder storage")?;
    h.release(100)?;
    h.finish_workers();
    drop(h.lease);
    assert_eq!(h.scope.snapshot().reserved_bytes, 2 * PROFILE_OPERATION_BYTES);
    assert_eq!(facts.units(), &[69, 58, 92, 112]);
    drop(facts);
    assert_eq!(h.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
