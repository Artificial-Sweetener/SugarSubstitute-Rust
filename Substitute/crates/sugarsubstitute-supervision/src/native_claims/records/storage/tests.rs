// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual publication/retry predicates with controlled native cleanup outcomes.
use super::*;
use sugarsubstitute_contracts::lifecycle::{
    CoordinatorEpoch, NamespaceId, ResourceGeneration, ResourceId,
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn record() -> Result<Record, Box<dyn std::error::Error>> {
    Ok(Record::backend(
        ResourceKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            id: ResourceId::new(1)?,
            generation: ResourceGeneration::new(1)?,
        },
        [3; 32],
    ))
}
#[test]
fn unknown_released_digest_or_kind_is_never_replaced() -> TestResult {
    let expected = record()?;
    let mut old = expected.clone();
    old.state = RecordState::Released;
    old.owner.epoch = CoordinatorEpoch::new([9; 16])?;
    old.resource = 8;
    old.generation = 9;
    assert!(permits_replacement(&Record::decode(&old.encode())?, &expected));
    old.digest = [4; 32];
    assert!(!permits_replacement(&Record::decode(&old.encode())?, &expected));
    let mut coordinator = Record::coordinator(expected.owner);
    coordinator.state = RecordState::Released;
    let mut matching_digest = expected.clone();
    matching_digest.digest = coordinator.digest;
    assert!(!permits_replacement(&Record::decode(&coordinator.encode())?, &matching_digest));
    old = expected.clone();
    old.owner.epoch = CoordinatorEpoch::new([9; 16])?;
    assert!(!permits_replacement(&Record::decode(&old.encode())?, &expected));
    old = expected.clone();
    old.generation = 2;
    assert!(!permits_replacement(&Record::decode(&old.encode())?, &expected));
    Ok(())
}

#[test]
fn only_checked_released_legacy_claim_can_advance_to_an_authenticated_custody_generation()
-> TestResult {
    let old = record()?;
    let original = old.encode();
    let mut current = old.clone();
    current.version = 2;
    current.custodian = 19;
    for state in [RecordState::Held, RecordState::LaunchPending, RecordState::Owned] {
        let mut active = old.clone();
        active.state = state;
        active.process = if state == RecordState::Owned { 7 } else { 0 };
        assert!(!permits_replacement(&active, &current));
    }
    let mut released = old.clone();
    released.state = RecordState::Released;
    assert!(permits_replacement(&released, &current));
    assert_eq!(old.encode(), original);
    assert!(!permits_replacement(&current, &old));
    let mut foreign = current.clone();
    foreign.custodian = 20;
    assert!(!permits_replacement(&current, &foreign));
    assert!(permits_replacement(&current, &current));
    Ok(())
}
#[test]
fn activated_matching_retry_preserves_fact_on_failure_and_finishes_without_new_activation()
-> TestResult {
    let bytes = record()?.encode();
    let mut state = Some(bytes);
    assert!(
        finish_activation(&mut state, &bytes, || Err(ClaimError::invalid("flush_failed"))).is_err()
    );
    assert_eq!(state, Some(bytes));
    assert!(
        finish_activation(&mut state, &bytes, || Err(ClaimError::invalid("close_failed"))).is_err()
    );
    assert_eq!(state, Some(bytes));
    let mut cleanups = 0;
    assert!(finish_activation(&mut state, &bytes, || {
        cleanups += 1;
        Ok(())
    })?);
    assert_eq!(cleanups, 1);
    assert!(state.is_none());
    assert!(!finish_activation(&mut state, &bytes, || {
        cleanups += 1;
        Ok(())
    })?);
    assert_eq!(cleanups, 1);
    Ok(())
}
#[test]
fn changed_desired_record_requires_previous_actual_activation_cleanup() -> TestResult {
    let held = record()?;
    let mut released = held.clone();
    released.state = RecordState::Released;
    let mut state = Some(held.encode());
    assert!(
        finish_activation(&mut state, &released.encode(), || Err(ClaimError::invalid(
            "still_owned"
        )))
        .is_err()
    );
    assert_eq!(state, Some(held.encode()));
    assert!(!finish_activation(&mut state, &released.encode(), || Ok(()))?);
    assert!(state.is_none());
    Ok(())
}
