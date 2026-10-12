// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Associate the exact suspended diagnostic root before resume; retain rejected native custody.
use super::*;
use std::os::windows::io::IntoRawHandle;
impl Family {
    /// Adopt both original diagnostic pins before an attribute list can borrow their section.
    pub(crate) fn adopt_diagnostic(
        &mut self,
        input: &mut WindowsLaunchInput,
    ) -> Result<(), RegistryError> {
        if let Some(launch) = &input.diagnostic {
            if self.fixture_events.is_some()
                || self.lifecycle_bootstrap.is_some()
                || self.output_pin.is_some()
                || self.attributes_initialized
                || self.diagnostic_bootstrap.is_some()
                || !launch.validates(launch.key)
            {
                return Err(RegistryError::invalid("diagnostic_inheritance_owner"));
            }
            let raw = launch.bootstrap.borrow_inheritance().section.as_raw_handle() as usize;
            if raw == 0 || raw >= usize::MAX - 15 {
                return Err(RegistryError::invalid("diagnostic_inheritance_handle"));
            }
        }
        if let Some(launch) = input.diagnostic.take() {
            self.diagnostic_key = Some(launch.key);
            self.diagnostic_enrollment = launch.enrollment;
            self.diagnostic_bootstrap = Some(launch.bootstrap);
        }
        Ok(())
    }
}
impl WindowsPort {
    /// Transfer the actual suspended duplicate only after exact session and binding checks.
    /// Rejection restores the duplicate and enrollment pin to contained native custody.
    pub(super) fn enroll_diagnostic_native(
        &self,
        family: &mut Family,
        binding: DiagnosticKey,
        key: ProcessKey,
    ) -> Result<(), RegistryError> {
        if family.diagnostic_key != Some(binding)
            || key.owner != binding.owner()
            || family.resumed
            || family
                .diagnostic_bootstrap
                .as_ref()
                .zip(family.diagnostic_enrollment.as_ref())
                .is_none_or(|(bootstrap, enrollment)| !bootstrap.matches_enrollment(enrollment))
        {
            return Err(RegistryError::invalid("diagnostic_enrollment_identity"));
        }
        let input = self.duplicate_native(family, 0)?;
        #[cfg(not(test))]
        let WatchInput::Native(process) = input;
        #[cfg(test)]
        let WatchInput::Native(process) = input else {
            return Err(RegistryError::invalid("diagnostic_enrollment_input"));
        };
        let Some(enrollment) = family.diagnostic_enrollment.take() else {
            family.watch_duplicate = Some(process.into_raw_handle() as usize);
            return Err(RegistryError::invalid("diagnostic_enrollment_missing"));
        };
        match enrollment.enroll(binding, key, process) {
            Ok(()) => Ok(()),
            Err(rejected) => {
                family.watch_duplicate = Some(rejected.process.into_raw_handle() as usize);
                family.diagnostic_enrollment = Some(rejected.enrollment);
                Err(RegistryError::new("diagnostic_enrollment_rejected", rejected.error))
            }
        }
    }
}
