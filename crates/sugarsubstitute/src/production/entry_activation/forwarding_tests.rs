// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fake recipient acceptance only; retained reply ownership and correlation stay real.
use super::*;
use sugarsubstitute_contracts::lifecycle::{
    CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RequestId, RequestOutcome,
    ResourceReply, RunGeneration,
};
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
fn transition(a: RunKey, b: RunKey) -> Result<ResourceTransition> {
    Ok(ResourceTransition {
        replies: vec![
            ResourceReply { run: a, request: RequestId::new(1)?, outcome: RequestOutcome::Failed },
            ResourceReply {
                run: b,
                request: RequestId::new(2)?,
                outcome: RequestOutcome::Cancelled,
            },
        ],
        effects: vec![],
    })
}
#[test]
fn foreign_reply_failure_preserves_remaining_delivery_and_does_not_repeat_accepted_recipient()
-> Result<()> {
    let a = run(1)?;
    let b = run(2)?;
    let original = transition(a, b)?;
    let mut forwarded = Forwarded::new(original.clone());
    let calls = std::cell::Cell::new(0);
    forwarded.deliver(a, |returned| {
        assert_eq!(returned, &original);
        calls.set(calls.get() + 1);
        Ok(())
    })?;
    assert!(
        forwarded
            .deliver(b, |returned| {
                assert_eq!(returned, &original);
                Err("actual recipient rejection".into())
            })
            .is_err()
    );
    assert!(!forwarded.complete());
    forwarded.deliver(a, |_| {
        calls.set(calls.get() + 1);
        Ok(())
    })?;
    assert_eq!(calls.get(), 1);
    forwarded.deliver(b, |returned| {
        assert_eq!(returned, &original);
        Ok(())
    })?;
    assert!(forwarded.complete());
    Ok(())
}
#[test]
fn foreign_reply_delivery_uses_complete_run_key_and_preserves_original_effect_data() -> Result<()> {
    let a = run(1)?;
    let b = run(2)?;
    let mut stale = a;
    stale.generation = RunGeneration::new(2)?;
    let mut original = transition(a, b)?;
    let resource = sugarsubstitute_contracts::lifecycle::ResourceKey {
        owner: a.owner,
        id: sugarsubstitute_contracts::lifecycle::ResourceId::new(1)?,
        generation: sugarsubstitute_contracts::lifecycle::ResourceGeneration::new(1)?,
    };
    original.effects.push(
        sugarsubstitute_contracts::lifecycle::ResourceEffect::DisconnectAttached { resource },
    );
    let mut forwarded = Forwarded::new(original.clone());
    let calls = std::cell::Cell::new(0);
    forwarded.deliver(stale, |_| {
        calls.set(calls.get() + 1);
        Ok(())
    })?;
    assert_eq!(calls.get(), 0);
    for recipient in [a, b] {
        forwarded.deliver(recipient, |returned| {
            assert_eq!(returned, &original);
            calls.set(calls.get() + 1);
            Ok(())
        })?;
    }
    assert_eq!(calls.get(), 2);
    assert!(forwarded.complete());
    Ok(())
}
