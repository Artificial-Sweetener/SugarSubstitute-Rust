// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow genuine original controls while native capture retains whole custody.

#[cfg(any(windows, test))]
use super::{
    CapturedEntryEnvironment, EntryEnvironmentFailure, EntryPreparationInputError,
    port::EnvironmentPort,
};
use super::{EntryPreparationError, owner::EntryPreparationOwner};
use sugarsubstitute_contracts::execution::{JobContext, Moment};

/// The existing admitted worker supplies genuine reconstructed original ports.
/// Public construction and arbitrary origin projection are absent.
pub struct EntryPreparationContext {
    owner: EntryPreparationOwner,
}
impl EntryPreparationContext {
    #[cfg(any(windows, test))]
    pub(super) fn new(owner: EntryPreparationOwner) -> Self {
        Self { owner }
    }
    /// Borrow actual original ports. Matching caller-created ports grant no origin.
    pub fn job_context(&self) -> &JobContext {
        self.owner.original_context()
    }
    /// Validate fresh publication against actual original cancellation and cutoff.
    /// # Errors
    /// Retains a genuine cause plus unchanged original custody. A fatal failure
    /// closes publication; mandatory post-effect observations remain independent.
    pub fn check(&self) -> Result<Moment, EntryPreparationError> {
        self.owner
            .check_publication()
            .map_err(|cause| EntryPreparationError::new(cause, self.owner.clone()))
    }
    /// Observe genuine original controls after an already-owned external outcome.
    /// Sticky publication closure does not suppress this mandatory observation.
    /// # Errors
    /// Returns the actual original cause with unchanged whole preparation custody.
    pub fn observe_control(&self) -> Result<Moment, EntryPreparationError> {
        self.owner
            .observe_control()
            .map_err(|cause| EntryPreparationError::new(cause, self.owner.clone()))
    }
    /// Copy the actual original Windows Unicode block once, then check its Free.
    /// # Errors
    /// Preserves first native/control cause and at most eight later causes. A failed
    /// Free remains unsettled under the same job and is never retried.
    #[cfg(windows)]
    pub fn capture_environment(&self) -> Result<CapturedEntryEnvironment, EntryEnvironmentFailure> {
        self.owner
            .check_native_layout(size_of::<crate::windows_entry_environment::WindowsEnvironment>())
            .map_err(|cause| EntryEnvironmentFailure::from_control(cause, self.owner.clone()))?;
        self.capture_port(Box::new(crate::windows_entry_environment::WindowsEnvironment::new()))
    }
    #[cfg(any(windows, test))]
    fn capture_port(
        &self,
        port: Box<dyn EnvironmentPort>,
    ) -> Result<CapturedEntryEnvironment, EntryEnvironmentFailure> {
        if let Err(cause) = self.owner.check_publication() {
            return Err(EntryEnvironmentFailure::from_control(cause, self.owner.clone()));
        }
        if let Err(cause) = self.owner.install_capture(port) {
            return Err(EntryEnvironmentFailure::from_control(cause, self.owner.clone()));
        }
        let mut access = self
            .owner
            .native_access()
            .map_err(|cause| EntryEnvironmentFailure::from_control(cause, self.owner.clone()))?;
        let Some(capture) = access.capture_mut() else {
            return Err(EntryEnvironmentFailure::from_control(
                EntryPreparationInputError::Closed,
                self.owner.clone(),
            ));
        };
        capture.capture(&mut || self.owner.native_control());
        match capture.take_units() {
            Some(units) => Ok(CapturedEntryEnvironment { units, owner: self.owner.clone() }),
            None => Err(EntryEnvironmentFailure::from_capture(capture, self.owner.clone())),
        }
    }
    #[cfg(test)]
    pub(crate) fn capture_with_port(
        &self,
        port: Box<dyn EnvironmentPort>,
    ) -> Result<CapturedEntryEnvironment, EntryEnvironmentFailure> {
        self.capture_port(port)
    }
}
