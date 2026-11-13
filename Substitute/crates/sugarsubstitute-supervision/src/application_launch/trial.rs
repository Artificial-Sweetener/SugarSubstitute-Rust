// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Creator ownership begins before a worker can publish a trial or construct its launch command.
use super::*;
use crate::setup_storage::{
    InstalledCandidate, PreparedCandidate, SignedCandidate, TrialTransfer, UnstartedTrial,
};

/// Automatic preparation rechecks repository authority; explicit installation retains its choice.
enum Authorization {
    Signed(Arc<SignedCandidate>),
    Installed(Arc<InstalledCandidate>),
}
impl Authorization {
    /// Payload custody has the same trial, creation and restoration owner for either choice.
    fn prepared(&self) -> &Arc<PreparedCandidate> {
        match self {
            Self::Signed(candidate) => candidate.prepared(),
            Self::Installed(candidate) => candidate.prepared(),
        }
    }

    /// Resolve the caller's explicit authority without allowing either path to impersonate it.
    fn transfer(
        &self,
        now: impl FnMut() -> i64,
        context: &JobContext,
        recovery: &mut Option<UnstartedTrial>,
    ) -> Result<TrialTransfer, ClaimError> {
        match self {
            Self::Signed(candidate) => candidate
                .transfer(now, context, recovery)
                .map_err(|cause| ClaimError::new("application_launch_trial_preparation", cause)),
            Self::Installed(candidate) => candidate.transfer(context, recovery).map_err(|cause| {
                ClaimError::new("application_launch_installed_preparation", cause)
            }),
        }
    }
}

/// Retained independently of worker results, including errors after durable trial publication.
pub(super) struct TrialLaunch {
    candidate: Option<Authorization>,
    prepared: Arc<PreparedCandidate>,
    recovery: Option<UnstartedTrial>,
    transfer: Option<TrialTransfer>,
}
impl TrialLaunch {
    /// Borrow only completed, retained native inheritance for command construction or creation.
    pub(super) fn transfer(&self) -> Result<&TrialTransfer, ClaimError> {
        self.transfer.as_ref().ok_or_else(|| ClaimError::invalid("application_launch_unprepared"))
    }

    /// Creation admission is closed by the outer owner before this storage operation.
    pub(super) fn restore(&self, context: &JobContext) -> Result<(), ClaimError> {
        if let Some(recovery) = &self.recovery {
            self.prepared.restore_unstarted(recovery, context).map_err(|cause| {
                ClaimError::new("application_launch_restore_preparation", cause)
            })?;
        }
        Ok(())
    }
}
impl ApplicationLaunch {
    /// Allocate inert ownership before admitting any trial work. Performs no native operation.
    /// Retain a clone outside the worker through preparation, creation and cleanup completion.
    pub fn trial(candidate: Arc<SignedCandidate>) -> Self {
        Self::authorized_trial(Authorization::Signed(candidate))
    }

    /// Retain a durable explicit installation choice before admitting its first health trial.
    /// Performs no native operation. Use the same prepare, create and recovery lifetime as trials.
    pub fn installed(candidate: Arc<InstalledCandidate>) -> Self {
        Self::authorized_trial(Authorization::Installed(candidate))
    }

    /// Establish creator custody before any worker can publish the chosen trial.
    fn authorized_trial(candidate: Authorization) -> Self {
        let launch = Self::empty();
        let prepared = Arc::clone(candidate.prepared());
        crate::native_claims::lock(&launch.0).custody =
            Some(LaunchCustody::Trial(Box::new(TrialLaunch {
                candidate: Some(candidate),
                prepared,
                recovery: None,
                transfer: None,
            })));
        launch
    }

    /// Reauthorize and prepare the exact trial on an admitted storage worker, retaining every
    /// publication prerequisite outside its result. Signed choices sample `now` in Unix UTC
    /// seconds; durable installation choices verify their retained local authorization. Reserve
    /// `SIGNED_TRANSFER_RESERVATION_BYTES`. After admission, only payload and recovery custody
    /// remain retained; signature-verification storage retires before native creation.
    /// This never creates an application process.
    /// # Errors
    /// Preserves trust, native custody and command errors. A busy run may be retried before
    /// admission; once trial publication is possible, use independent cleanup before another try.
    pub fn prepare_trial(
        &self,
        now: impl FnMut() -> i64,
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        let mut state = crate::native_claims::lock(&self.0);
        if state.issued || state.command.is_some() {
            return Err(ClaimError::invalid("application_launch_preparation_admission"));
        }
        let Some(LaunchCustody::Trial(trial)) = &mut state.custody else {
            return Err(ClaimError::invalid("application_launch_trial_custody"));
        };
        let transfer = trial
            .candidate
            .as_ref()
            .ok_or_else(|| ClaimError::invalid("application_launch_authorization_consumed"))?
            .transfer(now, context, &mut trial.recovery)?;
        trial.transfer = Some(transfer);
        trial.candidate = None;
        let transfer = trial.transfer()?;
        let command = WindowsLaunchCommand::from_os(
            &transfer.executable(),
            transfer.directory(),
            &transfer.arguments(),
        )
        .map_err(|cause| ClaimError::new("application_launch_command", cause))?;
        state.command = Some(command.into_native_parts());
        Ok(())
    }
}
