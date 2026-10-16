// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain Root's independent actual recovery-handler observer without transferring family authority.
use super::*;
use family_proof::RetainedFamilyProof;
use sugarsubstitute_contracts::{
    execution::{JobClass, JobCompletion, JobOutcome, Operation},
    recovery_supervisor::{RecoveryGrantKey, RecoveryProcessIdentity},
};
use sugarsubstitute_execution::RegisteredRecoveryGrant;
/// Prepare actual handler membership on Root, then bind only on one admitted finite worker.
/// Mutable ownership preserves the original permit before every fallible native effect.
pub struct RecoveryHandlerWatch {
    family: RetainedFamilyProof,
    grant: RegisteredRecoveryGrant,
    permit: Option<ProcessWatchPermit>,
    watch: Option<WatchJob>,
    actual: Option<RecoveryProcessIdentity>,
    terminal: Option<JobCompletion<ProcessExit, NativeObservationError>>,
    terminal_verified: bool,
    failure: Option<RegistryError>,
    terminal_failure: Option<RegistryError>,
}
/// Return the identical admitted input on every pre-effect handler preparation rejection.
pub struct RecoveryHandlerWatchRejection {
    /// Original real source/admission cause.
    pub cause: RegistryError,
    /// The actual original watch reservation, still requiring typed cancellation/observation.
    pub permit: ProcessWatchPermit,
}
impl WindowsProcessRegistry {
    /// Obtain one actual family loan before sending advisory PID/birth to a finite worker.
    /// Requires an unbounded-lifetime already-admitted native observer; no native work occurs here.
    /// # Errors
    /// Wrong Root/grant/Core, finite permit, stopped family or invalid ticket returns the input.
    pub fn prepare_recovery_handler_watch(
        &self,
        ticket: LaunchTicket,
        grant: &RegisteredRecoveryGrant,
        anchor: &super::recovery_member::RecoveryDesktopWatch,
        permit: ProcessWatchPermit,
    ) -> Result<RecoveryHandlerWatch, RecoveryHandlerWatchRejection> {
        let prepared = (|| {
            grant.check().map_err(|cause| RegistryError::new("recovery_handler_grant", cause))?;
            let execution = grant
                .execution_id()
                .map_err(|cause| RegistryError::new("recovery_handler_execution", cause))?;
            if !anchor.matches_family(self, ticket, grant)
                || ticket.binding != FamilyBinding::Desktop(grant.key().run())
                || self.owner != grant.key().run().owner
                || permit.owner_id() != execution
                || permit.deadline().is_some()
            {
                return Err(RegistryError::invalid("recovery_handler_admission"));
            }
            self.retained_family(ticket)
                .map_err(|cause| RegistryError::new("recovery_handler_family", cause))
        })();
        match prepared {
            Ok(family) => Ok(RecoveryHandlerWatch {
                family,
                grant: grant.clone(),
                permit: Some(permit),
                watch: None,
                actual: None,
                terminal: None,
                terminal_verified: false,
                failure: None,
                terminal_failure: None,
            }),
            Err(cause) => Err(RecoveryHandlerWatchRejection { cause, permit }),
        }
    }
}
impl RecoveryHandlerWatch {
    /// Return exact matching grant data; it cannot reconstruct native ownership.
    pub fn key(&self) -> RecoveryGrantKey {
        self.grant.key()
    }
    /// Observe actual kernel-verified handler identity after physical binding, not capture registration.
    pub const fn actual(&self) -> Option<RecoveryProcessIdentity> {
        self.actual
    }
    /// Bind the exact currently owned family member on the original finite Root context.
    /// No registration/healthy-capture fact is inferred from submitting or binding the watch.
    /// # Errors
    /// Source, native and partial-duplicate failures stay retained in this original owner/family.
    pub fn bind(
        &mut self,
        expected: RecoveryProcessIdentity,
        context: &JobContext,
    ) -> Result<(), RegistryError> {
        if let Some(cause) = &self.failure {
            return Err(cause.clone());
        }
        let result = (|| {
            let permit = self
                .permit
                .as_ref()
                .ok_or_else(|| RegistryError::invalid("recovery_handler_consumed"))?;
            if self.watch.is_some()
                || context.owner_id() != permit.owner_id()
                || context.spec().class != JobClass::InfrastructureLifecycle
                || context.spec().operation != Operation::NativeLifecycle
                || context.deadline() != Some(self.grant.original_budget().until())
                || context.cancellation_requested()
                || context.now() >= self.grant.original_budget().until()
            {
                return Err(RegistryError::invalid("recovery_handler_original_control"));
            }
            self.grant
                .check_context(context)
                .map_err(|cause| RegistryError::new("recovery_handler_grant", cause))?;
            let input = self.family.duplicate_member(
                Identity { pid: expected.pid(), birth: expected.birth() },
                context,
            )?;
            let permit = self
                .permit
                .take()
                .ok_or_else(|| RegistryError::invalid("recovery_handler_consumed"))?;
            self.watch = submission::bind(permit, input);
            if self.watch.is_none() {
                return Err(RegistryError::invalid("recovery_handler_binding"));
            }
            self.actual = Some(expected);
            Ok(())
        })();
        if let Err(cause) = &result {
            self.failure = Some(cause.clone());
        }
        result
    }
    /// Borrow the original binding failure; actual watch outcomes remain independent.
    pub const fn failure(&self) -> Option<&RegistryError> {
        self.failure.as_ref()
    }
    /// Cancel observation only; do not stop the process or revalidate an expired startup grant.
    pub fn cancel_observation(&mut self) {
        if let Some(permit) = self.permit.take() {
            self.watch = Some(permit.cancel_unbound());
        }
        if let Some(watch) = &self.watch {
            watch.cancel();
        }
    }
    /// Observe actual native callback/handle retirement without a startup-duration check.
    /// Cancellation/timeout is terminal observation, never physical process exit or family settlement.
    /// # Errors
    /// Retains first terminal metadata/source rejection and the entire real completion.
    pub fn observe_terminal(&mut self) -> Result<bool, RegistryError> {
        if let Some(cause) = &self.terminal_failure {
            return Err(cause.clone());
        }
        let Some(watch) = &mut self.watch else {
            return Ok(false);
        };
        if self.terminal.is_none() {
            self.terminal = watch.try_take_completion();
        }
        let Some(done) = &self.terminal else {
            return Ok(false);
        };
        let checked = if done.scope_id != watch.scope_id() || done.spec != watch.spec() {
            Err(RegistryError::invalid("recovery_handler_terminal_binding"))
        } else {
            match &done.outcome {
                JobOutcome::Success(exit) => self
                    .actual
                    .ok_or_else(|| RegistryError::invalid("recovery_handler_unbound_exit"))
                    .and_then(|actual| {
                        validate_recovery_exit(*exit, self.family.ticket.process, actual)
                    }),
                JobOutcome::Failure(cause) => {
                    Err(RegistryError::native_observation("recovery_handler_watch", cause.clone()))
                }
                JobOutcome::Cancelled | JobOutcome::TimedOut => Ok(()),
                JobOutcome::Panicked(_) | JobOutcome::Coalesced { .. } => {
                    Err(RegistryError::invalid("recovery_handler_nonexit"))
                }
            }
        };
        match checked {
            Ok(()) => {
                self.terminal_verified = true;
                Ok(true)
            }
            Err(cause) => {
                self.terminal_failure = Some(cause.clone());
                Err(cause)
            }
        }
    }
    /// Borrow exact typed terminal; cancellation is not proof that handler capture/flush finished.
    pub const fn terminal(&self) -> Option<&JobCompletion<ProcessExit, NativeObservationError>> {
        self.terminal.as_ref()
    }
    /// Observe native observer retirement independently of success, process exit or grant expiry.
    pub fn observation_retired(&self) -> bool {
        self.watch.as_ref().zip(self.terminal.as_ref()).is_some_and(|(watch, done)| {
            done.scope_id == watch.scope_id() && done.spec == watch.spec()
        })
    }
    /// Observe only actual independently verified signalled-process exit.
    pub fn actual_exit(&self) -> Option<ProcessExit> {
        if !self.terminal_verified {
            return None;
        }
        match &self.terminal.as_ref()?.outcome {
            JobOutcome::Success(exit) => Some(*exit),
            _ => None,
        }
    }
}
/// Match terminal identity and preserve every actual exit code, including259.
pub(super) fn validate_recovery_exit(
    exit: ProcessExit,
    process: sugarsubstitute_contracts::lifecycle::ProcessKey,
    actual: RecoveryProcessIdentity,
) -> Result<(), RegistryError> {
    if exit.correlation.get() != process.id.get()
        || exit.process_id != actual.pid()
        || exit.creation_time_100ns != actual.birth()
    {
        Err(RegistryError::invalid("recovery_physical_exit_identity"))
    } else {
        Ok(())
    }
}
impl std::fmt::Debug for RecoveryHandlerWatchRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecoveryHandlerWatchRejection { original owner retained }")
    }
}
impl std::fmt::Display for RecoveryHandlerWatchRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Recovery handler observation rejected")
    }
}
impl std::error::Error for RecoveryHandlerWatchRejection {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}
#[cfg(test)]
mod tests;
