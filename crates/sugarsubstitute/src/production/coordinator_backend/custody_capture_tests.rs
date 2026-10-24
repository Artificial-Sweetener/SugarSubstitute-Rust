// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject absent registered capture and expired custody ancestry before any native admission.
use super::{
    tests::{TestClock, Wake, key},
    *,
};
use std::sync::Arc;
use sugarsubstitute_contracts::coordinator_entry::EntrySetupBudget;
use sugarsubstitute_execution::ExecutionOwner;

#[test]
fn custody_requires_original_registered_capture_before_transport_or_process_admission() -> Result<()>
{
    let clock = Arc::new(TestClock::default());
    let execution = ExecutionOwner::new(clock.clone(), Arc::new(Wake))?;
    let client = execution.client();
    let scopes = ScopeSequence::new();
    let native =
        client.create_native_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, None)?;
    let cleanup =
        client.create_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, None)?;
    let jobs = JobSequence::new();
    let mut authority =
        CoordinatorAuthority::new(EntrySetupBudget::new(Moment::from_nanos(0))?, None)?;
    let mut resources = BackendResources {
        client: &client,
        native: &native,
        cleanup: &cleanup,
        jobs: &jobs,
        scopes: &scopes,
        authority: &mut authority,
        capture: None,
    };
    let before = native.snapshot();
    let original = Moment::from_nanos(1_000_000_000);
    for now in [Moment::from_nanos(0), original] {
        clock.set(now);
        let environment = LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?;
        assert!(
            super::super::coordinator_custody::CoordinatorCustody::new(
                key()?,
                "unopened-owned-host".into(),
                "unopened-owned-installation".into(),
                environment,
                original,
                &mut resources,
            )
            .is_err()
        );
        assert_eq!(native.snapshot(), before);
        assert!(resources.authority.registry().is_none());
        assert!(resources.authority.key().is_none());
    }
    assert_eq!(jobs.allocate()?, sugarsubstitute_contracts::execution::JobId::new(1)?);
    assert_eq!(scopes.allocate()?, sugarsubstitute_contracts::execution::ScopeId::new(3)?);
    Ok(())
}
