// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Enroll the independently contained custody helper before its original resume.
use super::*;
use std::os::windows::io::IntoRawHandle;
use sugarsubstitute_contracts::backend_custody::BackendCustodyKey;
impl Family {
    /// Move the exact authenticated session pins before native attribute borrowing.
    pub(crate) fn adopt_backend_custody(
        &mut self,
        input: &mut WindowsLaunchInput,
    ) -> Result<(), RegistryError> {
        if let Some(launch) = &input.backend_custody
            && (self.fixture_events.is_some()
                || self.lifecycle_bootstrap.is_some()
                || self.diagnostic_bootstrap.is_some()
                || self.output_pin.is_some()
                || self.attributes_initialized
                || self.backend_custody.is_some()
                || !launch.validates(launch.key))
        {
            return Err(RegistryError::invalid("custody_inheritance_owner"));
        }
        self.backend_custody = input.backend_custody.take();
        Ok(())
    }
}
impl WindowsPort {
    /// Adopt the same original suspended helper and restore all owners on rejection.
    pub(super) fn enroll_backend_custody_native(
        &self,
        family: &mut Family,
        binding: BackendCustodyKey,
        key: ProcessKey,
    ) -> Result<(), RegistryError> {
        if family.backend_custody.as_ref().is_none_or(|launch| !launch.validates(binding))
            || key.owner != binding.owner()
            || family.resumed
        {
            return Err(RegistryError::invalid("custody_enrollment_identity"));
        }
        let input = self.duplicate_native(family, 0)?;
        #[cfg(not(test))]
        let WatchInput::Native(process) = input;
        #[cfg(test)]
        let WatchInput::Native(process) = input else {
            return Err(RegistryError::invalid("custody_enrollment_input"));
        };
        let Some(launch) = &mut family.backend_custody else {
            family.watch_duplicate = Some(process.into_raw_handle() as usize);
            return Err(RegistryError::invalid("custody_enrollment_owner"));
        };
        let Some(enrollment) = launch.enrollment.take() else {
            family.watch_duplicate = Some(process.into_raw_handle() as usize);
            return Err(RegistryError::invalid("custody_enrollment_missing"));
        };
        match enrollment.enroll(binding, key, process) {
            Ok(()) => Ok(()),
            Err(rejected) => {
                family.watch_duplicate = Some(rejected.process.into_raw_handle() as usize);
                launch.enrollment = Some(rejected.enrollment);
                Err(RegistryError::new("custody_enrollment_rejected", rejected.error))
            }
        }
    }
}
