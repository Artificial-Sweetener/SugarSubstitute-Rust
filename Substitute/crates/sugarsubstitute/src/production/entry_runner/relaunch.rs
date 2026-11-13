// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Start the installation's entry once more after a restart to apply an update.
//!
//! The desktop child asks for this only after its close save finished. The runner acts only when
//! that child then completed a normal close and the coordinator released the installation, so
//! the new process starts against a free installation and its own launch picks the prepared
//! version. A crash, a failure or a close without that intent never relaunches.
use super::*;
use std::os::windows::ffi::OsStrExt;
use sugarsubstitute_contracts::execution::{
    JobClass, JobOutcome, JobSpec, Moment, Operation, OwnedJob,
};
use sugarsubstitute_supervision::{
    WindowsLaunchCommand,
    application_launch::{ApplicationLaunch, CREATION_RESERVATION_BYTES},
};

/// Progress of the one relaunch.
#[derive(Default)]
pub(super) enum Relaunch {
    /// Not started; it may never start.
    #[default]
    Idle,
    /// The launch job is running.
    Launching(OwnedJob<(), sugarsubstitute_supervision::ClaimError>),
    /// The launch finished, whatever its outcome; it is never retried.
    Done,
}

/// Whether the runner should relaunch: a normal shutdown with no failure, and a child that asked
/// to restart to apply and then completed a normal close.
pub(super) fn wanted(normal_shutdown: bool, failed: bool, restart_requested: bool) -> bool {
    normal_shutdown && !failed && restart_requested
}

impl<C: EntryActivation> EntryRunner<C> {
    /// Advance the relaunch; true once nothing more is owed. Call only after the installation
    /// was released. A failed launch is logged and never retried.
    /// # Errors
    /// Returns admission and clock failures of the launch job.
    pub(super) fn advance_relaunch(&mut self, until: Moment) -> Result<bool, Box<dyn Error>> {
        if matches!(self.relaunch, Relaunch::Idle) {
            let failed = self.failure_source().is_some();
            if !wanted(self.normal_shutdown, failed, self.child.restart_to_apply_requested()) {
                return Ok(true);
            }
            let entry =
                sugarsubstitute_configuration::layout::InstallationLayout::new(&self.installation)
                    .entry();
            let command = WindowsLaunchCommand::from_utf16(
                &entry.as_os_str().encode_wide().collect::<Vec<_>>(),
                &self.installation.as_os_str().encode_wide().collect::<Vec<_>>(),
                &[],
            )?;
            let launch = ApplicationLaunch::new(command);
            let client = self.client()?;
            let cleanup = self.cleanup.as_ref().ok_or("Production cleanup scope missing")?;
            let job = client.submit_blocking(
                cleanup,
                JobSpec {
                    id: self.jobs.allocate()?,
                    operation: Operation::Infrastructure,
                    class: JobClass::InfrastructureLifecycle,
                    deadline: Some(until),
                    reservation_bytes: CREATION_RESERVATION_BYTES,
                    replacement: None,
                },
                move |context| {
                    let launched = launch.launch(&context);
                    if launched.is_err() {
                        launch.retire()?;
                    }
                    launched
                },
            )?;
            tracing::info!(
                operation = "restart_to_apply_relaunch",
                "Starting SugarSubstitute again so the prepared update applies"
            );
            self.relaunch = Relaunch::Launching(job);
        }
        if let Relaunch::Launching(job) = &mut self.relaunch {
            let Some(completion) = job.try_take_completion() else { return Ok(false) };
            match completion.outcome {
                JobOutcome::Success(()) => {}
                JobOutcome::Failure(cause) => tracing::error!(
                    operation = "restart_to_apply_relaunch_failed",
                    cause = %cause,
                    "SugarSubstitute could not be started again; the update applies at the next start"
                ),
                other => tracing::error!(
                    operation = "restart_to_apply_relaunch_failed",
                    outcome = ?other.kind(),
                    "SugarSubstitute could not be started again; the update applies at the next start"
                ),
            }
            self.relaunch = Relaunch::Done;
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::wanted;

    #[test]
    fn only_intent_plus_a_normal_failure_free_shutdown_relaunches() {
        assert!(wanted(true, false, true));
        assert!(!wanted(true, false, false), "a normal close without intent");
        assert!(!wanted(false, false, true), "a crash or forced exit is not a normal shutdown");
        assert!(!wanted(true, true, true), "a failure during shutdown");
    }
}
