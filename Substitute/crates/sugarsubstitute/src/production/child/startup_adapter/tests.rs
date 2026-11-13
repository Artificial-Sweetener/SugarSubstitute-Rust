// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fence copied Comfy connection facts across actual mounted binding changes.

use super::*;
use crate::production_output::OutputBinding;
use sugarsubstitute_application::startup::{ConnectionBudgetKind, StartupPhase};
use sugarsubstitute_contracts::configuration::LoopbackOrigin;
use sugarsubstitute_supervision::lifecycle_protocol::BackendPeer;

fn backend_fixture(
    identity: MainIdentity,
) -> Result<(crate::startup::CanvasBackend, OutputBinding)> {
    let resource = ResourceKey {
        owner: identity.run.owner,
        id: ResourceId::new(1)?,
        generation: ResourceGeneration::new(1)?,
    };
    let lease = LeaseReference {
        resource: resource.id,
        generation: resource.generation,
        lease: LeaseId::new(1)?,
    };
    let binding =
        OutputBinding { run: identity.run, resource, connection: ConnectionId::new(1)?, lease };
    let peer = BackendPeer {
        process_id: std::num::NonZeroU32::new(47).ok_or("zero PID")?,
        creation_time_100ns: std::num::NonZeroU64::new(53).ok_or("zero process birth")?,
    };
    let backend = crate::startup::CanvasBackend {
        identity,
        lease,
        origin: LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?,
        peer,
    };
    Ok((backend, binding))
}

#[test]
fn attached_canvas_backend_uses_the_current_grant_without_managed_output() -> Result<()> {
    let (mut child, _owner, _clock) = test_support::child()?;
    let identity = child.identity;
    let (backend, _) = backend_fixture(identity)?;
    child.canvas_backend = Some(backend);
    assert!(child.canvas_backend(identity).is_none());
    child.phase = Some(StartupPhase::Running);
    child.connection_budget =
        Some(ConnectionBudget::new(ConnectionBudgetKind::Attached, Moment::from_nanos(0))?);
    child.lease = Some(backend.lease);
    assert_eq!(child.canvas_backend(identity), Some(backend));
    child.lease = None;
    assert!(child.canvas_backend(identity).is_none());
    child.lease = Some(backend.lease);
    let foreign = MainIdentity { navigation: Generation::new(2)?, ..identity };
    assert!(child.canvas_backend(foreign).is_none());
    Ok(())
}

#[test]
fn managed_canvas_backend_requires_current_healthy_main_and_lease() -> Result<()> {
    let (mut child, _owner, _clock) = test_support::child()?;
    let identity = child.identity;
    let (backend, binding) = backend_fixture(identity)?;
    let lease = backend.lease;
    child.canvas_backend = Some(backend);
    child.phase = Some(StartupPhase::Running);
    child.connection_budget =
        Some(ConnectionBudget::new(ConnectionBudgetKind::Managed, Moment::from_nanos(0))?);
    child.lease = Some(lease);
    assert!(child.canvas_backend(identity).is_none());
    child.running_backend = Some(backend::RunningBackendBinding::new(identity, binding)?);
    assert_eq!(child.canvas_backend(identity), Some(backend));
    let foreign = MainIdentity { navigation: Generation::new(2)?, ..identity };
    assert!(child.canvas_backend(foreign).is_none());

    let running = child.running_backend.as_mut().ok_or("Mounted backend absent")?;
    let failed = running.key(lease);
    assert!(running.unavailable(lease, false)?.is_some());
    assert!(child.canvas_backend(identity).is_none());
    let next = LeaseReference {
        resource: ResourceId::new(2)?,
        generation: ResourceGeneration::new(1)?,
        lease: LeaseId::new(2)?,
    };
    let running = child.running_backend.as_mut().ok_or("Mounted backend absent")?;
    assert_eq!(running.replacing(identity, lease, next)?, failed);
    running.replaced(identity, lease, next)?;
    assert_eq!(running.positive(next)?, Some(failed));
    assert!(child.canvas_backend(identity).is_none());
    child.canvas_backend = Some(crate::startup::CanvasBackend { lease: next, ..backend });
    child.lease = Some(next);
    assert_eq!(child.canvas_backend(identity).map(|current| current.lease), Some(next));
    Ok(())
}
