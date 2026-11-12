// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Transfer only an exact purpose bootstrap/enrollment pair after complete borrowed validation.
use super::*;
use sugarsubstitute_execution::purpose_pipe::PurposeKey;
use sugarsubstitute_execution::windows_purpose_pipe::{PurposeEnrollment, PurposePipeBootstrap};
/// Retain the exact original session pins until suspended native ownership is adopted.
pub(crate) struct PurposeLaunch {
    pub(crate) bootstrap: PurposePipeBootstrap,
    pub(crate) enrollment: Option<PurposeEnrollment>,
    pub(crate) key: PurposeKey,
}
impl PurposeLaunch {
    /// Require both actual pin provenance and the unchanged trusted composing identity.
    pub(crate) fn validates(&self, key: PurposeKey) -> bool {
        self.key == key
            && self.bootstrap.key() == key
            && self
                .enrollment
                .as_ref()
                .is_some_and(|enrollment| self.bootstrap.matches_enrollment(enrollment))
    }
}
impl WindowsLaunchInput {
    /// Assemble the original readonly bootstrap and its actual suspended-process enrollment.
    /// Validation borrows every owner; errors leave this input and both pins unchanged.
    /// Only success takes both pins. This performs no allocation, admission or native effect.
    ///
    /// # Errors
    /// Rejects absent or foreign-session pins and purpose/lifecycle/output/fixture conflicts.
    pub fn with_purpose_pipe(
        &mut self,
        bootstrap: &mut Option<PurposePipeBootstrap>,
        enrollment: &mut Option<PurposeEnrollment>,
    ) -> Result<(), RegistryInputError> {
        if self.purpose.is_some()
            || self.backend_custody.is_some()
            || self.lifecycle.is_some()
            || self.output.is_some()
            || self.fixture_events.is_some()
        {
            return Err(RegistryInputError::PurposeConflict);
        }
        if bootstrap
            .as_ref()
            .zip(enrollment.as_ref())
            .is_none_or(|(bootstrap, enrollment)| !bootstrap.matches_enrollment(enrollment))
        {
            return Err(RegistryInputError::PurposeProvenance);
        }
        // Both Options are exclusively borrowed and verified before this infallible transfer.
        if let (Some(bootstrap), Some(enrollment)) = (bootstrap.take(), enrollment.take()) {
            let key = enrollment.key();
            self.purpose = Some(PurposeLaunch { bootstrap, enrollment: Some(enrollment), key });
        }
        Ok(())
    }
}
