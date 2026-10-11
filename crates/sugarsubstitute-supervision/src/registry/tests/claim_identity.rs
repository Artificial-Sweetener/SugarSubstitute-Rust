// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove registry affinity and rejected-generation retirement through real execution.

use super::*;
use sugarsubstitute_contracts::lifecycle::{ResourceGeneration, ResourceId, ResourceKey};

#[test]
fn retained_family_records_fit_the_native_composition_stack_budget() -> TestResult {
    assert!(std::mem::size_of::<WindowsProcessRegistry>() <= 4096);
    let first = WindowsProcessRegistry::new(owner()?)?;
    let second = WindowsProcessRegistry::new(owner()?)?;
    assert_eq!(first.records.len(), 128);
    assert!(first.records.iter().all(Option::is_none));
    assert!(!std::ptr::eq(first.records.as_ptr(), second.records.as_ptr()));
    assert!(first.is_empty() && second.is_empty());
    Ok(())
}

#[test]
fn matching_coordinator_facts_do_not_substitute_for_retained_registry_identity() -> TestResult {
    let first = WindowsProcessRegistry::new(owner()?)?;
    let second = WindowsProcessRegistry::new(owner()?)?;
    let pinned = first.claim_identity(owner()?)?;
    assert!(pinned.same_as(&first.claim_identity(owner()?)?));
    assert!(pinned.clone().same_as(&pinned));
    assert!(!pinned.same_as(&second.claim_identity(owner()?)?));
    let foreign = CoordinatorKey { epoch: CoordinatorEpoch::new([2; 16])?, ..owner()? };
    assert!(matches!(first.claim_identity(foreign), Err(RegistryAdmissionError::ForeignOwner)));
    drop(first);
    assert!(!pinned.same_as(&WindowsProcessRegistry::new(owner()?)?.claim_identity(owner()?)?));
    Ok(())
}

#[test]
fn exact_rejected_managed_generation_remains_until_actual_watch_retirement() -> TestResult {
    let mut h = Harness::new()?;
    let resource = ResourceKey {
        owner: owner()?,
        id: ResourceId::new(1)?,
        generation: ResourceGeneration::new(1)?,
    };
    h.scope.cancel();
    let launch = input()?;
    let watch = h.spec(JobClass::NativeObservation, 4096)?;
    let create = h.spec(JobClass::InfrastructureLifecycle, launch.required_reservation_bytes())?;
    assert!(matches!(
        h.registry.submit_launch(
            &h.client,
            &h.scope,
            ScopeSpec { id: ScopeId::new(2)?, deadline: None },
            create,
            watch,
            FamilyBinding::Managed(resource),
            launch,
        ),
        Err(RegistryAdmissionError::Execution(_))
    ));
    let retained = h.registry.retained_managed(resource)?.ok_or("Rejected record lost")?;
    assert_eq!(retained.binding, FamilyBinding::Managed(resource));
    assert!(!retained.settled);
    assert!(
        h.registry
            .retained_managed(ResourceKey { generation: ResourceGeneration::new(2)?, ..resource })?
            .is_none()
    );
    let foreign = ResourceKey {
        owner: CoordinatorKey { epoch: CoordinatorEpoch::new([2; 16])?, ..resource.owner },
        ..resource
    };
    assert!(matches!(
        h.registry.retained_managed(foreign),
        Err(RegistryAdmissionError::ForeignOwner)
    ));
    h.drain_until(WindowsProcessRegistry::is_empty)?;
    assert!(h.registry.retained_managed(resource)?.is_none());
    assert_eq!(h.fake.creates.load(Ordering::Acquire), 0);
    Ok(())
}
