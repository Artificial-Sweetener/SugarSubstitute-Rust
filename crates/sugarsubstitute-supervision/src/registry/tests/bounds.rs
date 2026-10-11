// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Independently bound managed and desktop families without imposing process caps.
use super::*;
use sugarsubstitute_contracts::lifecycle::{ResourceGeneration, ResourceId, ResourceKey};

#[test]
fn sixty_four_managed_and_desktop_families_share_no_kind_quota() -> TestResult {
    let mut h = Harness::new()?;
    for instance in 1..=64 {
        let key = h.launch(instance)?.process();
        h.created(key)?;
    }
    for resource in 1..=64 {
        let launch = input()?;
        let watch = h.spec(JobClass::NativeObservation, 4096)?;
        let create =
            h.spec(JobClass::InfrastructureLifecycle, launch.required_reservation_bytes())?;
        let native = ScopeSpec { id: ScopeId::new(h.next_scope)?, deadline: None };
        h.next_scope += 1;
        let binding = FamilyBinding::Managed(ResourceKey {
            owner: owner()?,
            id: ResourceId::new(resource)?,
            generation: ResourceGeneration::new(1)?,
        });
        let key = h
            .registry
            .submit_launch(&h.client, &h.scope, native, create, watch, binding, launch)?
            .process();
        h.created(key)?;
    }
    assert_eq!(h.registry.records.iter().flatten().count(), 128);
    assert_eq!(h.fake.creates.load(Ordering::Acquire), 128);
    assert!(h.launch(65).is_err());
    assert_eq!(h.fake.creates.load(Ordering::Acquire), 128);
    Ok(())
}
