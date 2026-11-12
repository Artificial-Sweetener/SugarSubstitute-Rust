// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Associate the exact suspended purpose helper root before resume; retain rejected native custody.
use super::*;
use std::os::windows::io::IntoRawHandle;
impl Family {
    /// Adopt both original purpose pins before an attribute list can borrow their section.
    pub(crate) fn adopt_purpose(
        &mut self,
        input: &mut WindowsLaunchInput,
    ) -> Result<(), RegistryError> {
        if let Some(launch) = &input.purpose {
            if self.fixture_events.is_some()
                || self.lifecycle_bootstrap.is_some()
                || self.output_pin.is_some()
                || self.attributes_initialized
                || self.purpose_bootstrap.is_some()
                || self.backend_custody.is_some()
                || !launch.validates(launch.key)
            {
                return Err(RegistryError::invalid("diagnostic_inheritance_owner"));
            }
            let raw = launch.bootstrap.borrow_inheritance().section.as_raw_handle() as usize;
            if raw == 0 || raw >= usize::MAX - 15 {
                return Err(RegistryError::invalid("diagnostic_inheritance_handle"));
            }
        }
        if let Some(launch) = input.purpose.take() {
            self.purpose_key = Some(launch.key);
            self.purpose_enrollment = launch.enrollment;
            self.purpose_bootstrap = Some(launch.bootstrap);
        }
        Ok(())
    }
}
impl WindowsPort {
    /// Transfer the actual suspended duplicate only after exact session and binding checks.
    /// Rejection restores the duplicate and enrollment pin to contained native custody.
    pub(super) fn enroll_purpose_native(
        &self,
        family: &mut Family,
        binding: PurposeKey,
        key: ProcessKey,
    ) -> Result<(), RegistryError> {
        if family.purpose_key != Some(binding)
            || key.owner != binding.owner()
            || family.resumed
            || family
                .purpose_bootstrap
                .as_ref()
                .zip(family.purpose_enrollment.as_ref())
                .is_none_or(|(bootstrap, enrollment)| !bootstrap.matches_enrollment(enrollment))
        {
            return Err(RegistryError::invalid("purpose_enrollment_identity"));
        }
        let input = self.duplicate_native(family, 0)?;
        #[cfg(not(test))]
        let WatchInput::Native(process) = input;
        #[cfg(test)]
        let WatchInput::Native(process) = input else {
            return Err(RegistryError::invalid("purpose_enrollment_input"));
        };
        let Some(enrollment) = family.purpose_enrollment.take() else {
            family.watch_duplicate = Some(process.into_raw_handle() as usize);
            return Err(RegistryError::invalid("purpose_enrollment_missing"));
        };
        match enrollment.enroll(binding, key, process) {
            Ok(()) => Ok(()),
            Err(rejected) => {
                family.watch_duplicate = Some(rejected.process.into_raw_handle() as usize);
                family.purpose_enrollment = Some(rejected.enrollment);
                Err(RegistryError::new("purpose_enrollment_rejected", rejected.error))
            }
        }
    }
}
