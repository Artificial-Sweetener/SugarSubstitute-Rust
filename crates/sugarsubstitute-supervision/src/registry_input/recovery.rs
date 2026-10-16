// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the genuine Root sections and carrier enrollment through native creation.
use super::*;
use sugarsubstitute_execution::{
    RegisteredRecoveryGrant,
    windows_recovery_supervisor::{RecoveryCarrierBootstrap, RecoveryEnrollment},
};
pub(crate) struct RecoveryCarrierLaunch {
    pub(crate) bootstrap: RecoveryCarrierBootstrap,
    pub(crate) enrollment: Option<RecoveryEnrollment>,
    pub(crate) grant: RegisteredRecoveryGrant,
}
impl WindowsLaunchInput {
    /// Bind the actual Root recovery/UI sections and their exact suspended carrier enrollment.
    /// The profile owner separately supplies the genuine inherited Job token in argv.
    /// # Errors
    /// Rejects stopped/foreign grant, unmatched actual bootstrap/session or buffer capacities.
    pub fn with_recovery_carrier(
        command: WindowsLaunchCommand,
        environment: LaunchEnvironment,
        bootstrap: RecoveryCarrierBootstrap,
        enrollment: RecoveryEnrollment,
        grant: &RegisteredRecoveryGrant,
    ) -> Result<Self, RegistryInputError> {
        if grant.check().is_err()
            || bootstrap.key() != grant.key()
            || !bootstrap.matches_enrollment(&enrollment)
        {
            return Err(RegistryInputError::LifecycleProvenance);
        }
        let mut value = Self::new(command, environment)?;
        value.recovery_carrier = Some(Box::new(RecoveryCarrierLaunch {
            bootstrap,
            enrollment: Some(enrollment),
            grant: grant.clone(),
        }));
        Ok(value)
    }
}
