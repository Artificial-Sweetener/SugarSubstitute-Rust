// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Transfer only an exact diagnostic bootstrap/enrollment pair after complete borrowed validation.
use super::*;
use sugarsubstitute_contracts::incident::DiagnosticKey;
use sugarsubstitute_execution::windows_diagnostic_pipe::{
    DiagnosticEnrollment, DiagnosticPipeBootstrap,
};
/// Retain the exact original session pins until suspended native ownership is adopted.
pub(crate) struct DiagnosticLaunch {
    pub(crate) bootstrap: DiagnosticPipeBootstrap,
    pub(crate) enrollment: Option<DiagnosticEnrollment>,
    pub(crate) key: DiagnosticKey,
}
impl DiagnosticLaunch {
    /// Require both actual pin provenance and the unchanged trusted composing identity.
    pub(crate) fn validates(&self, key: DiagnosticKey) -> bool {
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
    /// Rejects absent or foreign-session pins and diagnostic/lifecycle/output/fixture conflicts.
    pub fn with_diagnostic_pipe(
        &mut self,
        bootstrap: &mut Option<DiagnosticPipeBootstrap>,
        enrollment: &mut Option<DiagnosticEnrollment>,
    ) -> Result<(), RegistryInputError> {
        if self.diagnostic.is_some()
            || self.lifecycle.is_some()
            || self.output.is_some()
            || self.fixture_events.is_some()
        {
            return Err(RegistryInputError::DiagnosticConflict);
        }
        if bootstrap
            .as_ref()
            .zip(enrollment.as_ref())
            .is_none_or(|(bootstrap, enrollment)| !bootstrap.matches_enrollment(enrollment))
        {
            return Err(RegistryInputError::DiagnosticProvenance);
        }
        // Both Options are exclusively borrowed and verified before this infallible transfer.
        if let (Some(bootstrap), Some(enrollment)) = (bootstrap.take(), enrollment.take()) {
            let key = enrollment.key();
            self.diagnostic =
                Some(DiagnosticLaunch { bootstrap, enrollment: Some(enrollment), key });
        }
        Ok(())
    }
}
