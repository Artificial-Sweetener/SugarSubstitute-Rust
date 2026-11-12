// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Enroll the exact suspended root without losing rejected native ownership.
use super::*;
use std::os::windows::io::IntoRawHandle;

impl Family {
    /// Retain both channel pins before any native attribute can borrow a section.
    pub(crate) fn adopt_lifecycle(
        &mut self,
        input: &mut WindowsLaunchInput,
    ) -> Result<(), RegistryError> {
        if input.lifecycle.is_some()
            && (self.fixture_events.is_some()
                || self.attributes_initialized
                || self.lifecycle_bootstrap.is_some()
                || self.purpose_bootstrap.is_some()
                || self.backend_custody.is_some())
        {
            return Err(RegistryError::invalid("lifecycle_inheritance_owner"));
        }
        if let Some(launch) = input.lifecycle.take() {
            let raw = launch.bootstrap.borrow_inheritance().section.as_raw_handle() as usize;
            if raw == 0 || raw >= usize::MAX - 15 {
                return Err(RegistryError::invalid("lifecycle_inheritance_handle"));
            }
            self.lifecycle_run = Some(launch.run);
            self.lifecycle_enrollment = launch.enrollment;
            self.lifecycle_bootstrap = Some(launch.bootstrap);
        }
        Ok(())
    }
}
impl WindowsPort {
    pub(super) fn enroll_native(
        &self,
        family: &mut Family,
        run: RunKey,
        key: ProcessKey,
    ) -> Result<(), RegistryError> {
        if family.recovery_carrier.is_some() {
            return self.enroll_recovery_carrier(family, run, key);
        }
        if family.lifecycle_run.is_none() {
            return Ok(());
        }
        if family.lifecycle_run != Some(run) || key.owner != run.owner {
            return Err(RegistryError::invalid("lifecycle_enrollment_identity"));
        }
        // The first duplicate is already transferred to the root watch. Any partial
        // second duplicate remains in the same durable checked-close family slot.
        let input = self.duplicate_native(family, 0)?;
        #[cfg(not(test))]
        let WatchInput::Native(process) = input;
        #[cfg(test)]
        let WatchInput::Native(process) = input else {
            return Err(RegistryError::invalid("lifecycle_enrollment_input"));
        };
        let Some(enrollment) = family.lifecycle_enrollment.take() else {
            // Restore ownership before reporting an invariant error; no native Drop.
            family.watch_duplicate = Some(process.into_raw_handle() as usize);
            return Err(RegistryError::invalid("lifecycle_enrollment_missing"));
        };
        match enrollment.enroll(run, key, process) {
            Ok(()) => Ok(()),
            Err(rejected) => {
                family.watch_duplicate = Some(rejected.process.into_raw_handle() as usize);
                family.lifecycle_enrollment = Some(rejected.enrollment);
                Err(RegistryError::new("lifecycle_enrollment_rejected", rejected.error))
            }
        }
    }
}
