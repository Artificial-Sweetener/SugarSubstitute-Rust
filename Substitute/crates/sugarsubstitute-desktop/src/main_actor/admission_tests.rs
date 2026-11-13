// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify exact-current admission and retirement without native effects.

use super::*;
use std::error::Error;
use sugarsubstitute_contracts::{
    execution::Generation,
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    startup::LaunchId,
};

pub(super) fn identity() -> Result<MainIdentity, Box<dyn Error>> {
    Ok(MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        },
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    })
}
#[test]
fn environment_and_browser_require_separate_exact_current_authorizations()
-> Result<(), Box<dyn Error>> {
    let identity = identity()?;
    let mut owner = Admission::new(identity);
    assert!(owner.authorize_environment(identity).is_err());
    assert!(owner.authorize_browser(identity).is_err());
    owner.environment_observed()?;
    assert!(owner.controller_observed(42).is_err());
    owner.authorize_environment(identity)?;
    assert!(owner.authorize_browser(identity).is_err());
    assert!(owner.controller_observed(0).is_err());
    owner.controller_observed(42)?;
    assert!(owner.frontend(identity).is_err());
    owner.authorize_browser(identity)?;
    assert!(!owner.hook_completed(true)?);
    assert!(owner.navigation_admitted().is_err());
    assert!(owner.hook_completed(false)?);
    owner.navigation_admitted()?;
    owner.frontend(identity)?;
    assert!(owner.authorize_environment(identity).is_err());
    assert!(owner.authorize_browser(identity).is_err());
    Ok(())
}
#[test]
fn every_identity_component_is_checked_before_native_admission() -> Result<(), Box<dyn Error>> {
    let expected = identity()?;
    let mut stale = [expected; 4];
    stale[0].launch = LaunchId::new(2)?;
    stale[1].navigation = Generation::new(2)?;
    stale[2].run.generation = RunGeneration::new(2)?;
    stale[3].run.owner.epoch = CoordinatorEpoch::new([3; 16])?;
    for foreign in stale {
        let mut owner = Admission::new(expected);
        owner.environment_observed()?;
        assert!(owner.authorize_environment(foreign).is_err());
        owner.authorize_environment(expected)?;
        owner.controller_observed(42)?;
        assert!(owner.authorize_browser(foreign).is_err());
        owner.authorize_browser(expected)?;
        owner.hook_completed(true)?;
        owner.hook_completed(false)?;
        owner.navigation_admitted()?;
        assert!(owner.frontend(foreign).is_err());
        owner.frontend(expected)?;
    }
    Ok(())
}
#[test]
fn close_at_each_construction_stage_permanently_rejects_late_progress() -> Result<(), Box<dyn Error>>
{
    for phase in [
        Phase::ConstructingEnvironment,
        Phase::Environment,
        Phase::ConstructingController,
        Phase::Browser,
        Phase::Hooks,
        Phase::Frontend,
    ] {
        let identity = identity()?;
        let mut owner = Admission::new(identity);
        owner.phase = phase;
        owner.close();
        owner.close();
        assert!(owner.closing());
        assert!(owner.environment_observed().is_err());
        assert!(owner.authorize_environment(identity).is_err());
        assert!(owner.controller_observed(42).is_err());
        assert!(owner.authorize_browser(identity).is_err());
        assert!(owner.hook_completed(true).is_err());
        assert!(owner.navigation_admitted().is_err());
        assert!(owner.frontend(identity).is_err());
    }
    Ok(())
}

#[test]
fn frontend_timeout_diagnosis_requires_exact_issued_navigation_and_closing_suppresses_duplicates()
-> Result<(), Box<dyn Error>> {
    let expected = identity()?;
    let mut owner = Admission::new(expected);
    assert!(owner.timeout_diagnostic_allowed(expected).is_err());
    owner.environment_observed()?;
    owner.authorize_environment(expected)?;
    owner.controller_observed(42)?;
    owner.authorize_browser(expected)?;
    owner.hook_completed(true)?;
    owner.hook_completed(false)?;
    owner.navigation_admitted()?;
    assert!(owner.timeout_diagnostic_allowed(expected)?);
    let mut foreign = expected;
    foreign.navigation = Generation::new(2)?;
    assert!(owner.timeout_diagnostic_allowed(foreign).is_err());
    owner.close();
    assert!(!owner.timeout_diagnostic_allowed(expected)?);
    assert!(owner.timeout_diagnostic_allowed(foreign).is_err());
    Ok(())
}
