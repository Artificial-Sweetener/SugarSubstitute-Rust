// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Construct the native parent actor only under its same original projection charge.
use super::*;
impl ProductionActivation {
    // Parent return/Box temporaries live only for this bounded construction step.
    #[inline(never)]
    pub(super) fn construct_parent(
        start: &mut DesktopStart,
        parent: &mut Option<Box<ParentLifecycle>>,
        preparation: &mut Option<BackendPreparation>,
        causes: &mut Option<crate::production::parent_lifecycle::causes::BackendCauses>,
    ) -> Result<()> {
        if causes.is_none() {
            let charge = start.projection.memory.claim(ParentLifecycle::storage_bytes())?;
            *causes =
                Some(crate::production::parent_lifecycle::causes::BackendCauses::new(charge)?);
        }
        let original = start.original.ok_or("Actual prepared bootstrap deadline missing")?;
        *parent = Some(construct_with_causes(
            start.run,
            start.launch,
            start.value()?.profile.clone(),
            start.connection,
            original,
            causes,
        )?);
        let value = start.value_mut()?;
        *preparation = Some(BackendPreparation {
            source: value.frontend.take().ok_or("Original frontend input already transferred")?,
            environment: value.backend_environment.take(),
        });
        Ok(())
    }
}

/// Construct only inert actor state under the same original charged owner.
/// Caller supplies its genuine bootstrap; shape-only owner tests grant no peer or native authority.
pub(super) fn construct_with_causes(
    run: sugarsubstitute_contracts::lifecycle::RunKey,
    launch: sugarsubstitute_contracts::startup::LaunchId,
    profile: sugarsubstitute_contracts::configuration::ProfileId,
    connection: sugarsubstitute_contracts::lifecycle::ConnectionId,
    original: sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeDeadline,
    causes: &mut Option<crate::production::parent_lifecycle::causes::BackendCauses>,
) -> Result<Box<ParentLifecycle>> {
    Ok(Box::new(ParentLifecycle::new(run, launch, profile, connection, original, causes)?))
}
