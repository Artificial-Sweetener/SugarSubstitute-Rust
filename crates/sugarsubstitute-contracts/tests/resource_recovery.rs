// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify public outage facts retain exact identities and original owned causes.
use std::error::Error;
use sugarsubstitute_contracts::{configuration::LoopbackOrigin, lifecycle::*};

fn lease(id: u64, generation: u64) -> Result<LeaseKey, IdentityError> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([7; 32]),
        epoch: CoordinatorEpoch::new([8; 16])?,
    };
    Ok(LeaseKey {
        run: RunKey { owner, instance: InstanceId::new(3)?, generation: RunGeneration::new(4)? },
        resource: ResourceKey {
            owner,
            id: ResourceId::new(5)?,
            generation: ResourceGeneration::new(generation)?,
        },
        connection: ConnectionId::new(6)?,
        id: LeaseId::new(id)?,
    })
}

#[test]
fn managed_exit_retains_original_registry_status_and_redacted_owned_cause()
-> Result<(), Box<dyn Error>> {
    let original = lease(9, 10)?;
    let process = ProcessKey { owner: original.run.owner, id: ProcessId::new(11)? };
    let cause = Box::new(std::io::Error::other("private-original-exit-detail"));
    let source = std::ptr::from_ref(cause.as_ref());
    let observation = ResourceObservation::ManagedExited {
        resource: original.resource,
        process,
        status: u32::MAX,
        failure: LifecycleFailure::new(LifecycleFailureCode::UnexpectedExit, cause),
    };
    assert_eq!(observation.resource(), original.resource);
    assert!(!format!("{observation:?}").contains("private-original-exit-detail"));
    let ResourceObservation::ManagedExited { resource, process: actual, status, failure } =
        observation
    else {
        return Err("Managed exit observation kind changed".into());
    };
    assert_eq!(resource, original.resource);
    assert_eq!(actual, process);
    assert_eq!(status, u32::MAX);
    assert_eq!(failure.code(), LifecycleFailureCode::UnexpectedExit);
    let retained = failure
        .source()
        .and_then(|value| value.downcast_ref::<std::io::Error>())
        .ok_or("Original exit cause type lost")?;
    assert_eq!(std::ptr::from_ref(retained), source);
    assert_eq!(retained.to_string(), "private-original-exit-detail");
    Ok(())
}

#[test]
fn retry_command_duplicate_matching_keeps_every_original_lease_component()
-> Result<(), IdentityError> {
    let original = lease(u64::MAX, 10)?;
    let command = ResourceCommand::RetryDead(original);
    assert_eq!(command.clone(), ResourceCommand::RetryDead(original));
    let other_run = RunKey { generation: original.run.generation.next()?, ..original.run };
    let other_resource =
        ResourceKey { generation: original.resource.generation.next()?, ..original.resource };
    for changed in [
        LeaseKey { run: other_run, ..original },
        LeaseKey { resource: other_resource, ..original },
        LeaseKey { connection: ConnectionId::new(12)?, ..original },
        LeaseKey { id: LeaseId::new(13)?, ..original },
    ] {
        assert_ne!(command, ResourceCommand::RetryDead(changed));
    }
    assert_ne!(command, ResourceCommand::Release(original));
    assert_ne!(command, ResourceCommand::Reload(original));
    Ok(())
}

#[test]
fn resource_lookup_keeps_exact_acquisition_for_existing_observation_kinds()
-> Result<(), Box<dyn Error>> {
    let original = lease(9, 10)?;
    let process = ProcessKey { owner: original.run.owner, id: ProcessId::new(11)? };
    let observations = [
        ResourceObservation::ClaimAcquired(original.resource),
        ResourceObservation::OwnedStarted { resource: original.resource, process },
        ResourceObservation::Ready {
            resource: original.resource,
            origin: LoopbackOrigin::parse("http://localhost:8188", 8188)?,
        },
        ResourceObservation::Failed {
            resource: original.resource,
            failure: LifecycleFailure::new(
                LifecycleFailureCode::Preparation,
                Box::new(std::io::Error::other("original preparation")),
            ),
        },
        ResourceObservation::Settled(original.resource),
    ];
    for observation in observations {
        assert_eq!(observation.resource(), original.resource);
    }
    Ok(())
}

#[test]
fn transition_keeps_cohort_pairing_with_original_reply_and_no_implicit_effects()
-> Result<(), IdentityError> {
    let previous = lease(9, 10)?;
    let current = lease(12, 11)?;
    let mut transition = ResourceTransition::default();
    assert!(transition.replies.is_empty());
    assert!(transition.effects.is_empty());
    assert!(transition.invalidations.is_empty());
    assert!(transition.replacements.is_empty());
    let invalidation = GrantedResourceInvalidation { lease: previous, status: u32::MAX };
    let replacement = GrantedResourceReplacement { previous, current };
    transition.invalidations.push(invalidation);
    transition.replacements.push(replacement);
    transition.replies.push(ResourceReply {
        run: previous.run,
        request: RequestId::new(14)?,
        outcome: RequestOutcome::Granted(current),
    });
    let retained = transition.clone();
    assert_eq!(retained, transition);
    assert_eq!(retained.invalidations, vec![invalidation]);
    assert_eq!(retained.replacements, vec![replacement]);
    assert_eq!(retained.replies[0].outcome, RequestOutcome::Granted(current));
    assert!(retained.effects.is_empty());
    transition.replacements[0].previous.connection = ConnectionId::new(15)?;
    assert_ne!(retained, transition);
    assert_eq!(retained.replacements[0].previous, previous);
    assert_eq!(retained.replacements[0].current, current);
    assert_eq!(retained.invalidations[0].status, u32::MAX);
    Ok(())
}
