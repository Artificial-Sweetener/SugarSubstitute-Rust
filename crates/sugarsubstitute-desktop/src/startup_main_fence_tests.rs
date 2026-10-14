// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual creation fence with typed external custody facts, without windows.
use super::*;
use sugarsubstitute_contracts::{execution::Generation, lifecycle::*, startup::LaunchId};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn identity(launch: u64) -> Result<MainIdentity, Box<dyn std::error::Error>> {
    Ok(MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        },
        launch: LaunchId::new(launch)?,
        navigation: Generation::new(launch)?,
    })
}
#[test]
fn exact_absence_fences_queued_and_future_same_attempt_creation_before_observation() -> TestResult {
    let id = identity(1)?;
    let mut fence = MainFence::new();
    let queued_before_check = id;
    assert!(fence.prepare(id, Some(id)).is_ok());
    fence.absent(id, Some(id), false, MainCustody::Absent)?;
    assert!(fence.prepare(queued_before_check, Some(id)).is_err());
    assert!(fence.prepare(id, Some(id)).is_err());
    assert!(fence.absent(id, Some(id), false, MainCustody::Absent).is_err());
    Ok(())
}
#[test]
fn live_partial_callback_and_application_constructor_custody_prevent_absence() -> TestResult {
    let id = identity(1)?;
    let mut fence = MainFence::new();
    // The adapter samples any retained actor as Outstanding until its actual checked
    // retirement, covering live, partial constructor and in-flight callback custody alike.
    for _stage in ["live", "partial constructor", "callback pending"] {
        assert!(fence.absent(id, Some(id), false, MainCustody::Outstanding).is_err());
        assert!(fence.prepare(id, Some(id)).is_ok());
    }
    assert!(fence.absent(id, Some(id), true, MainCustody::Absent).is_err());
    assert!(fence.absent(id, Some(id), false, MainCustody::CheckedRetired(id)).is_err());
    assert!(fence.prepare(id, Some(id)).is_ok());
    Ok(())
}
#[test]
fn stale_foreign_and_unregistered_successor_identity_cannot_close_current_attempt() -> TestResult {
    let current = identity(2)?;
    let mut fence = MainFence::new();
    let mut foreign = current;
    foreign.run.instance = InstanceId::new(2)?;
    for requested in [identity(1)?, identity(3)?, foreign] {
        assert!(fence.absent(requested, Some(current), false, MainCustody::Absent).is_err());
    }
    assert!(fence.absent(current, None, false, MainCustody::Absent).is_err());
    fence.absent(current, Some(current), false, MainCustody::CheckedRetired(identity(1)?))?;
    assert!(fence.prepare(current, Some(current)).is_err());
    assert!(fence.prepare(identity(3)?, Some(identity(3)?)).is_ok());
    Ok(())
}
#[test]
fn callback_quiescence_and_checked_predecessor_order_are_required_before_successor_absence()
-> TestResult {
    let prior = identity(1)?;
    let current = identity(2)?;
    let mut fence = MainFence::new();
    assert!(fence.absent(current, Some(current), false, MainCustody::Outstanding).is_err());
    assert!(
        fence
            .absent(current, Some(current), false, MainCustody::CheckedRetired(identity(3)?))
            .is_err()
    );
    fence.absent(current, Some(current), false, MainCustody::CheckedRetired(prior))?;
    assert!(fence.prepare(current, Some(current)).is_err());
    Ok(())
}
