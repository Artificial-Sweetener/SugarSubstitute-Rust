// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Use real charged delivery storage and fake only recipient/producer boundaries.
use super::*;
use std::cell::Cell;
use sugarsubstitute_contracts::lifecycle::*;
fn run(instance: u64) -> Result<RunKey> {
    Ok(RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(instance)?,
        generation: RunGeneration::new(1)?,
    })
}
fn owned() -> Result<TransitionDelivery> {
    let claim = crate::production::coordinator_backend::tests::storage_tests::claim(
        TransitionDelivery::storage_bytes(),
    )
    .map_err(super::super::outer)?;
    TransitionDelivery::new(claim)
}
#[test]
fn failed_later_recipient_keeps_actual_return_and_blocks_another_producer() -> Result<()> {
    let a = run(1)?;
    let b = run(2)?;
    let mut owner = owned()?;
    let original = ResourceTransition {
        replies: vec![
            ResourceReply { run: a, request: RequestId::new(1)?, outcome: RequestOutcome::Failed },
            ResourceReply {
                run: b,
                request: RequestId::new(2)?,
                outcome: RequestOutcome::Cancelled,
            },
        ],
        effects: vec![],
    };
    owner.produce(|| Ok(original.clone()))?;
    let accepted = Cell::new(0);
    assert!(
        owner
            .deliver(|held| {
                held.deliver(a, |_| {
                    accepted.set(accepted.get() + 1);
                    Ok(())
                })?;
                held.deliver(b, |_| Err("actual recipient failed".into()))
            })
            .is_err()
    );
    assert!(owner.pending());
    let invoked = Cell::new(false);
    assert!(
        owner
            .produce(|| {
                invoked.set(true);
                Ok(ResourceTransition::default())
            })
            .is_err()
    );
    assert!(!invoked.get());
    owner.deliver(|held| {
        held.deliver(a, |_| {
            accepted.set(accepted.get() + 1);
            Ok(())
        })?;
        held.deliver(b, |actual| {
            assert_eq!(actual, &original);
            Ok(())
        })
    })?;
    assert_eq!(accepted.get(), 1);
    assert!(!owner.pending());
    Ok(())
}
#[test]
fn producer_failure_retains_pending_return_without_overwriting_prior_owner() -> Result<()> {
    let mut owner = owned()?;
    let a = run(1)?;
    let actual = ResourceTransition {
        replies: vec![ResourceReply {
            run: a,
            request: RequestId::new(1)?,
            outcome: RequestOutcome::Failed,
        }],
        effects: vec![],
    };
    assert!(owner.produce(|| Err("actual producer failed after its effects".into())).is_err());
    owner.capture_pending(|| Some(actual.clone()));
    let source_called = Cell::new(false);
    owner.capture_pending(|| {
        source_called.set(true);
        Some(ResourceTransition::default())
    });
    assert!(!source_called.get());
    owner.deliver(|held| {
        held.deliver(a, |returned| {
            assert_eq!(returned, &actual);
            Ok(())
        })
    })?;
    assert!(!owner.pending());
    Ok(())
}
