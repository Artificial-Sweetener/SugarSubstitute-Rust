// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Transfer only an exact backend_custody bootstrap/enrollment pair after complete borrowed validation.
use super::*;
use sugarsubstitute_contracts::backend_custody::BackendCustodyKey;
use sugarsubstitute_execution::windows_backend_custody::{
    BackendCustodyBootstrap, BackendCustodyEnrollment,
};
/// Retain the exact original session pins until suspended native ownership is adopted.
pub(crate) struct CustodyLaunch {
    pub(crate) bootstrap: BackendCustodyBootstrap,
    pub(crate) enrollment: Option<BackendCustodyEnrollment>,
    pub(crate) key: BackendCustodyKey,
}
impl CustodyLaunch {
    /// Require both actual pin provenance and the unchanged trusted composing identity.
    pub(crate) fn validates(&self, key: BackendCustodyKey) -> bool {
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
    /// Rejects absent or foreign-session pins and backend_custody/lifecycle/output/fixture conflicts.
    pub fn with_backend_custody_pipe(
        &mut self,
        bootstrap: &mut Option<BackendCustodyBootstrap>,
        enrollment: &mut Option<BackendCustodyEnrollment>,
    ) -> Result<(), RegistryInputError> {
        self.transfer_backend_custody(bootstrap, enrollment, false)
    }
    /// Transfer the genuine captured backend server and its suspended-helper enrollment.
    /// The captured marker and actual session allocation must both match before either
    /// pin moves. The original capture attachment remains in that admitted session.
    ///
    /// # Errors
    /// Returns provenance/conflict errors without changing this input or either pin.
    pub fn with_backend_custody_capture(
        &mut self,
        bootstrap: &mut Option<BackendCustodyBootstrap>,
        enrollment: &mut Option<BackendCustodyEnrollment>,
    ) -> Result<(), RegistryInputError> {
        self.transfer_backend_custody(bootstrap, enrollment, true)
    }
    /// Borrow all original owners before committing one purpose-specific pair.
    fn transfer_backend_custody(
        &mut self,
        bootstrap: &mut Option<BackendCustodyBootstrap>,
        enrollment: &mut Option<BackendCustodyEnrollment>,
        captured: bool,
    ) -> Result<(), RegistryInputError> {
        if self.backend_custody.is_some()
            || self.diagnostic.is_some()
            || self.lifecycle.is_some()
            || self.output.is_some()
            || self.fixture_events.is_some()
        {
            return Err(RegistryInputError::BackendCustodyConflict);
        }
        if bootstrap.as_ref().zip(enrollment.as_ref()).is_none_or(|(bootstrap, enrollment)| {
            !bootstrap.matches_enrollment(enrollment)
                || bootstrap.captures(enrollment.key()) != captured
        }) {
            return Err(RegistryInputError::BackendCustodyProvenance);
        }
        // Both Options are exclusively borrowed and verified before this infallible transfer.
        if let (Some(bootstrap), Some(enrollment)) = (bootstrap.take(), enrollment.take()) {
            let key = enrollment.key();
            self.backend_custody =
                Some(CustodyLaunch { bootstrap, enrollment: Some(enrollment), key });
        }
        Ok(())
    }
}
