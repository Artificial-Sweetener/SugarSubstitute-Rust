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
    ) -> Result<()> {
        let charge = start.projection.memory.claim(ParentLifecycle::storage_bytes())?;
        let original = start.original.ok_or("Actual prepared bootstrap deadline missing")?;
        *parent = Some(Box::new(ParentLifecycle::new(
            start.run,
            start.launch,
            start.value()?.profile.clone(),
            start.connection,
            original,
            charge,
        )?));
        let value = start.value_mut()?;
        *preparation = Some(BackendPreparation {
            source: value.frontend.take().ok_or("Original frontend input already transferred")?,
            environment: value.backend_environment.take(),
        });
        Ok(())
    }
}
