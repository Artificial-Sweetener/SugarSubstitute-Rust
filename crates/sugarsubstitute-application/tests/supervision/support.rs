// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real lease, profile, replay and actual-settlement owners through public contracts.
pub(super) use std::{error::Error, num::NonZeroU16};
pub(super) use sugarsubstitute_application::supervision::*;
pub(super) use sugarsubstitute_contracts::{
    configuration::{LaunchConfiguration, LoopbackOrigin, ProfileId},
    lifecycle::*,
    startup::LaunchId,
};
pub(super) type TestResult = Result<(), Box<dyn Error>>;

pub(super) fn owner() -> Result<CoordinatorKey, IdentityError> {
    Ok(CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    })
}
pub(super) fn run(instance: u64) -> Result<RunKey, IdentityError> {
    Ok(RunKey {
        owner: owner()?,
        instance: InstanceId::new(instance)?,
        generation: RunGeneration::new(1)?,
    })
}
pub(super) fn connection(instance: u64) -> Result<ConnectionId, IdentityError> {
    ConnectionId::new(instance)
}
pub(super) fn register(
    coordinator: &mut ResourceCoordinator,
    instance: u64,
    profile: &str,
) -> TestResult {
    let registration = ClientRegistration {
        run: run(instance)?,
        launch: LaunchId::new(instance)?,
        profile: ProfileId::new(profile.into())?,
        connection: connection(instance)?,
    };
    coordinator.enroll_run(RunEnrollment {
        run: registration.run,
        launch: registration.launch,
        profile: registration.profile.clone(),
    })?;
    coordinator.register_run(registration)?;
    Ok(())
}
pub(super) fn target(port: u16, managed: bool) -> Result<BackendTarget, Box<dyn Error>> {
    let port = NonZeroU16::new(port).ok_or("Zero test port")?;
    let root = std::env::temp_dir().join("lifecycle-test-installation");
    Ok(if managed {
        BackendTarget::Managed(LaunchConfiguration::new(
            root.clone(),
            root.join("python"),
            vec!["--actual".into()],
            port.get(),
        )?)
    } else {
        BackendTarget::Attached(LoopbackOrigin::parse(
            &format!("http://127.0.0.1:{port}"),
            port.get(),
        )?)
    })
}
pub(super) fn request(
    instance: u64,
    id: u64,
    command: ResourceCommand,
) -> Result<ResourceRequest, IdentityError> {
    Ok(ResourceRequest {
        run: run(instance)?,
        connection: connection(instance)?,
        id: RequestId::new(id)?,
        command,
    })
}
pub(super) fn submit(
    coordinator: &mut ResourceCoordinator,
    request: ResourceRequest,
) -> Result<ResourceTransition, LifecycleRejection> {
    // This fixture supplies trusted peer facts independently of the decoded command.
    coordinator.request(ClientBinding { run: request.run, connection: request.connection }, request)
}
pub(super) fn lease(transition: &ResourceTransition) -> Result<LeaseKey, Box<dyn Error>> {
    match transition.replies.last().map(|reply| reply.outcome) {
        Some(RequestOutcome::Pending(lease) | RequestOutcome::Granted(lease)) => Ok(lease),
        _ => Err("Missing lease".into()),
    }
}
pub(super) fn observe(
    coordinator: &mut ResourceCoordinator,
    observation: ResourceObservation,
) -> Result<ResourceTransition, LifecycleRejection> {
    coordinator.observe(observation).map_err(|error| error.reason)
}
pub(super) fn ready(
    coordinator: &mut ResourceCoordinator,
    lease: LeaseKey,
    port: u16,
    managed: bool,
) -> TestResult {
    if managed {
        observe(coordinator, ResourceObservation::ClaimAcquired(lease.resource))?;
        observe(
            coordinator,
            ResourceObservation::OwnedStarted {
                resource: lease.resource,
                process: ProcessKey {
                    owner: owner()?,
                    id: ProcessId::new(lease.resource.id.get())?,
                },
            },
        )?;
    }
    observe(
        coordinator,
        ResourceObservation::Ready {
            resource: lease.resource,
            origin: LoopbackOrigin::parse(&format!("http://127.0.0.1:{port}"), port)?,
        },
    )?;
    Ok(())
}
