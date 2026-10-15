// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise genuine same-table prepared reservation and original unbound-watch retirement.
use super::*;
use sugarsubstitute_contracts::backend_custody::{BackendCustodyKey, BackendCustodySession};
#[test]
fn prepared_profile_reserves_same_desktop_slot_and_closes_only_original_watch_before_reuse()
-> TestResult {
    let mut h = Harness::new()?;
    let FamilyBinding::Desktop(run) = binding(1)? else {
        return Err("Desktop fixture absent".into());
    };
    let parent = BackendCustodyKey::new(run.owner, BackendCustodySession::new(1)?);
    let watch = h.spec(JobClass::NativeObservation, 4096)?;
    let native = ScopeSpec { id: ScopeId::new(h.next_scope)?, deadline: None };
    h.next_scope += 1;
    let (_, profile, _) =
        h.registry.reserve_profile_family(run, parent, &h.client, native, watch)?;
    assert_eq!(h.registry.records.iter().flatten().count(), 1);
    let retained = h.registry.retained_desktop(run)?.ok_or("Prepared row absent")?;
    assert_eq!(retained.process.id.get(), profile.registry_process());
    assert!(!retained.started && !retained.settled);
    assert_eq!(h.fake.creates.load(Ordering::Acquire), 0);
    let rejected_watch = h.spec(JobClass::NativeObservation, 4096)?;
    assert!(matches!(
        h.registry.reserve_profile_family(
            run,
            parent,
            &h.client,
            ScopeSpec { id: ScopeId::new(h.next_scope)?, deadline: None },
            rejected_watch
        ),
        Err(RegistryAdmissionError::Busy)
    ));
    assert!(!h.registry.retire_prepared_profile(run)?);
    h.drain_until(|registry| registry.records.iter().all(Option::is_none))?;
    assert!(h.registry.retire_prepared_profile(run)?);
    assert_eq!(h.fake.creates.load(Ordering::Acquire), 0);
    Ok(())
}
#[test]
fn foreign_or_invalid_profile_reservation_never_occupies_original_table_or_watch() -> TestResult {
    let mut h = Harness::new()?;
    let FamilyBinding::Desktop(run) = binding(1)? else {
        return Err("Desktop fixture absent".into());
    };
    let foreign = BackendCustodyKey::new(
        CoordinatorKey { namespace: NamespaceId::new([7; 32]), ..run.owner },
        BackendCustodySession::new(1)?,
    );
    let watch = h.spec(JobClass::NativeObservation, 4096)?;
    let native = ScopeSpec { id: ScopeId::new(h.next_scope)?, deadline: None };
    assert!(matches!(
        h.registry.reserve_profile_family(run, foreign, &h.client, native, watch),
        Err(RegistryAdmissionError::ForeignOwner)
    ));
    let parent = BackendCustodyKey::new(run.owner, BackendCustodySession::new(1)?);
    assert!(matches!(
        h.registry.reserve_profile_family(
            run,
            parent,
            &h.client,
            ScopeSpec { deadline: Some(Moment::from_nanos(1)), ..native },
            watch
        ),
        Err(RegistryAdmissionError::InvalidSpecification)
    ));
    assert!(h.registry.is_empty());
    assert_eq!(h.fake.creates.load(Ordering::Acquire), 0);
    assert_eq!(h.client.create_scope(native, None)?.id(), native.id);
    Ok(())
}
