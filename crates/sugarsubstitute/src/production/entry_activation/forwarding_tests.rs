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
        invalidations: vec![],
        replacements: vec![],
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
    forwarded.deliver(a, |returned, _| {
        assert_eq!(returned, &original);
        calls.set(calls.get() + 1);
        Ok(())
    })?;
    assert!(
        forwarded
            .deliver(b, |returned, _| {
                assert_eq!(returned, &original);
                Err("actual recipient rejection".into())
            })
            .is_err()
    );
    assert!(!forwarded.complete());
    forwarded.deliver(a, |_, _| {
        calls.set(calls.get() + 1);
        Ok(())
    })?;
    assert_eq!(calls.get(), 1);
    forwarded.deliver(b, |returned, _| {
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
    forwarded.deliver(stale, |_, _| {
        calls.set(calls.get() + 1);
        Ok(())
    })?;
    assert_eq!(calls.get(), 0);
    for recipient in [a, b] {
        forwarded.deliver(recipient, |returned, _| {
            assert_eq!(returned, &original);
            calls.set(calls.get() + 1);
            Ok(())
        })?;
    }
    assert_eq!(calls.get(), 2);
    assert!(forwarded.complete());
    Ok(())
}

fn lease_for(
    run: RunKey,
    id: u64,
    generation: u64,
) -> Result<sugarsubstitute_contracts::lifecycle::LeaseKey> {
    use sugarsubstitute_contracts::lifecycle::*;
    Ok(LeaseKey {
        run,
        connection: ConnectionId::new(1)?,
        id: LeaseId::new(id)?,
        resource: ResourceKey {
            owner: run.owner,
            id: ResourceId::new(1)?,
            generation: ResourceGeneration::new(generation)?,
        },
    })
}
#[test]
fn invalidation_only_and_replacement_only_returns_retain_every_actual_recipient() -> Result<()> {
    use sugarsubstitute_contracts::lifecycle::{
        GrantedResourceInvalidation, GrantedResourceReplacement,
    };
    let a = run(1)?;
    let b = run(2)?;
    for replacement in [false, true] {
        let mut original = ResourceTransition::default();
        for (owner, id) in [(a, 1), (b, 2)] {
            let old = lease_for(owner, id, 1)?;
            if replacement {
                original.replacements.push(GrantedResourceReplacement {
                    previous: old,
                    current: lease_for(owner, id + 2, 2)?,
                });
            } else {
                original
                    .invalidations
                    .push(GrantedResourceInvalidation { lease: old, status: 259 });
            }
        }
        let mut held = Forwarded::new(original.clone());
        held.capture_contexts_with(|pair| {
            Ok(Some(ReadyContext {
                resource: pair.current.resource,
                document: MountedDocument::new(
                    sugarsubstitute_contracts::configuration::LoopbackOrigin::parse(
                        "http://127.0.0.1:8188",
                        8188,
                    )?,
                    "1.0",
                    "1.0",
                )
                .map_err(super::super::outer)?,
            }))
        })?;
        assert!(!held.complete());
        held.deliver(a, |value, _| {
            assert_eq!(value, &original);
            Ok(())
        })?;
        assert!(!held.complete());
        assert!(held.deliver(b, |_, _| Err("actual recipient rejection".into())).is_err());
        assert!(!held.complete());
        held.deliver(b, |value, _| {
            assert_eq!(value, &original);
            Ok(())
        })?;
        assert!(held.complete());
    }
    Ok(())
}
#[test]
fn pending_native_handoff_is_backpressure_without_acceptance_or_error_normalization() -> Result<()>
{
    use sugarsubstitute_contracts::lifecycle::GrantedResourceInvalidation;
    let owner = run(1)?;
    let original = ResourceTransition {
        invalidations: vec![GrantedResourceInvalidation {
            lease: lease_for(owner, 1, 1)?,
            status: 1,
        }],
        ..ResourceTransition::default()
    };
    let mut held = Forwarded::new(original.clone());
    held.deliver(owner, |_, _| {
        Err(crate::production::parent_lifecycle::ParentError::TransitionPending.into())
    })?;
    assert!(!held.complete());
    assert!(
        held.deliver(owner, |_, _| Err(
            crate::production::parent_lifecycle::ParentError::ForeignPeer.into()
        ))
        .is_err()
    );
    assert!(!held.complete());
    held.deliver(owner, |value, _| {
        assert_eq!(value, &original);
        Ok(())
    })?;
    assert!(held.complete());
    Ok(())
}
#[test]
fn all256_notices_use_original64_recipient_slots_and_exact_run_generations() -> Result<()> {
    use sugarsubstitute_contracts::lifecycle::GrantedResourceInvalidation;
    let mut original = ResourceTransition::default();
    for instance in 1..=64 {
        for interest in 0..4 {
            original.invalidations.push(GrantedResourceInvalidation {
                lease: lease_for(run(instance)?, instance * 4 + interest, 1)?,
                status: 1,
            });
        }
    }
    let mut held = Forwarded::new(original.clone());
    let calls = std::cell::Cell::new(0);
    let mut stale = run(1)?;
    stale.generation = RunGeneration::new(2)?;
    held.deliver(stale, |_, _| {
        calls.set(calls.get() + 1);
        Ok(())
    })?;
    assert_eq!(calls.get(), 0);
    for instance in 1..=64 {
        held.deliver(run(instance)?, |value, _| {
            assert_eq!(value, &original);
            calls.set(calls.get() + 1);
            Ok(())
        })?;
    }
    assert_eq!(calls.get(), 64);
    assert!(held.complete());
    Ok(())
}

#[test]
fn missing_or_foreign_producer_context_keeps_raw_replacement_and_blocks_acceptance() -> Result<()> {
    use sugarsubstitute_contracts::lifecycle::GrantedResourceReplacement;
    let owner = run(1)?;
    let pair = GrantedResourceReplacement {
        previous: lease_for(owner, 1, 1)?,
        current: lease_for(owner, 2, 2)?,
    };
    let original = ResourceTransition { replacements: vec![pair], ..ResourceTransition::default() };
    let mut held = Forwarded::new(original.clone());
    assert!(held.capture_contexts_with(|_| Ok(None)).is_err());
    assert!(!held.complete());
    assert!(held.deliver(owner, |_, _| Err("recipient must remain uninvoked".into())).is_err());
    assert_eq!(held.transition, original);
    let foreign_resource = ResourceKey {
        generation: sugarsubstitute_contracts::lifecycle::ResourceGeneration::new(3)?,
        ..pair.current.resource
    };
    assert!(
        held.capture_contexts_with(|_| Ok(Some(ReadyContext {
            resource: foreign_resource,
            document: MountedDocument::new(
                sugarsubstitute_contracts::configuration::LoopbackOrigin::parse(
                    "http://127.0.0.1:8188",
                    8188
                )?,
                "1.0",
                "1.0"
            )
            .map_err(super::super::outer)?,
        })))
        .is_err()
    );
    assert_eq!(held.transition, original);
    assert!(!held.complete());
    held.capture_contexts_with(|actual| {
        Ok(Some(ReadyContext {
            resource: actual.current.resource,
            document: MountedDocument::new(
                sugarsubstitute_contracts::configuration::LoopbackOrigin::parse(
                    "http://127.0.0.1:8188",
                    8188,
                )?,
                "1.0",
                "1.0",
            )
            .map_err(super::super::outer)?,
        }))
    })?;
    held.deliver(owner, |value, contexts| {
        assert_eq!(value, &original);
        contexts.document(pair)?;
        Ok(())
    })?;
    assert!(held.complete());
    Ok(())
}
