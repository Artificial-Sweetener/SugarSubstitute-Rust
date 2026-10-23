// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Independently enroll the exact still-suspended recovery Desktop in Root's physical family.
//! Carrier membership, actual UI lifetime and handler lifetime remain distinct facts.
use super::*;
use family_proof::RetainedFamilyProof;
use sugarsubstitute_contracts::{
    execution::{JobClass, JobCompletion, JobOutcome, Operation},
    recovery_supervisor::{RecoveryGrantKey, RecoveryProcessIdentity},
};
use sugarsubstitute_execution::RegisteredRecoveryGrant;

/// Pre-admitted independent UI watch; no native child or matching PID manufactures this owner.
/// Cancellation retains its actual typed terminal. Dropping cannot prove retirement.
#[must_use = "Bind the exact suspended Desktop or cancel and observe this watch"]
pub struct RecoveryDesktopWatch {
    identity: RegistryIdentity,
    ticket: LaunchTicket,
    grant: RegisteredRecoveryGrant,
    permit: Option<ProcessWatchPermit>,
    family: Option<RetainedFamilyProof>,
    failed: bool,
}
/// Actual Root-local family/member ownership, distinct from authenticated relay statements.
/// Private construction follows exact kernel birth/membership and real watch binding.
#[must_use = "Retain Root's independent UI watch through terminal and paired family cleanup"]
pub struct RecoveryEnrolledDesktop {
    family: RetainedFamilyProof,
    grant: RegisteredRecoveryGrant,
    actual: RecoveryProcessIdentity,
    watch: WatchJob,
    terminal: Option<JobCompletion<ProcessExit, NativeObservationError>>,
    lifecycle_enrolled: bool,
    terminal_verified: bool,
    terminal_rejection: Option<RegistryError>,
    failure: Option<RegistryError>,
}
impl WindowsProcessRegistry {
    /// Reserve actual independent UI observation before the durable recovery ACK.
    /// Reuses the original profile/carrier native scope and unchanged execution quotas.
    /// A fresh allocation is admitted once for this exact genuine Root grant/family.
    /// # Errors
    /// Rejects foreign/stopped grants, wrong real profile family, repeated reservation,
    /// finite watches or queue/memory saturation before any child/path/kernel effect.
    pub fn reserve_recovery_desktop_watch(
        &mut self,
        execution: &ExecutionClient,
        ticket: LaunchTicket,
        grant: &RegisteredRecoveryGrant,
        spec: JobSpec,
    ) -> Result<RecoveryDesktopWatch, RegistryAdmissionError> {
        grant.check().map_err(RegistryAdmissionError::Execution)?;
        if grant.key().run().owner != self.owner
            || ticket.binding != FamilyBinding::Desktop(grant.key().run())
        {
            return Err(RegistryAdmissionError::ForeignOwner);
        }
        submission::validate_watch(spec)?;
        let index = self.index(ticket.process)?;
        let record = self.records[index].as_mut().ok_or(RegistryAdmissionError::Stale)?;
        if record.ticket != ticket
            || record.recovery_watch_issued
            || record.settled
            || record.shared.admission.load(Ordering::Acquire) == CLOSED
            || !self.launch_open
        {
            return Err(RegistryAdmissionError::Closed);
        }
        let permit = execution
            .reserve_process_watch(
                &record.native_scope,
                spec,
                sugarsubstitute_execution::windows::NativeCorrelation::new(ticket.process.id.get())
                    .map_err(|cause| {
                        RegistryAdmissionError::Execution(
                            sugarsubstitute_contracts::execution::AdmissionError::Unexpected(cause),
                        )
                    })?,
            )
            .map_err(RegistryAdmissionError::Execution)?;
        record.recovery_watch_issued = true;
        Ok(RecoveryDesktopWatch {
            identity: self.identity.clone(),
            ticket,
            grant: grant.clone(),
            permit: Some(permit),
            family: None,
            failed: false,
        })
    }
}
impl RecoveryDesktopWatch {
    /// Match actual registry, retained family allocation and the same genuine grant.
    pub(crate) fn matches_family(
        &self,
        registry: &WindowsProcessRegistry,
        ticket: LaunchTicket,
        grant: &RegisteredRecoveryGrant,
    ) -> bool {
        self.identity.same_as(&registry.identity)
            && self.ticket == ticket
            && self.grant.same_grant(grant)
    }

    /// Return exact matching facts without granting another watch or family.
    pub fn key(&self) -> RecoveryGrantKey {
        self.grant.key()
    }
    /// Cancel only the original unbound observer while retaining all family/source custody.
    /// # Errors
    /// Returns the original consumed-permit rejection without dropping this owner.
    pub fn cancel_observation(&mut self) -> Result<WatchJob, RegistryError> {
        self.permit
            .take()
            .map(ProcessWatchPermit::cancel_unbound)
            .ok_or_else(|| RegistryError::invalid("recovery_member_watch_consumed"))
    }
    /// Retain the actual started family on the native actor before submitting finite work.
    /// This is an allocation/custody fence only; no process or path effects occur here.
    /// # Errors
    /// Rejects foreign registry, stopped grant, repeated preparation or an unstarted family.
    pub fn prepare_binding(
        &mut self,
        registry: &WindowsProcessRegistry,
    ) -> Result<(), RegistryError> {
        if self.failed
            || self.family.is_some()
            || self.permit.is_none()
            || !self.identity.same_as(&registry.identity)
        {
            return Err(RegistryError::invalid("recovery_member_preparation"));
        }
        self.grant.check().map_err(|cause| RegistryError::new("recovery_member_grant", cause))?;
        self.family = Some(
            registry
                .retained_family(self.ticket)
                .map_err(|cause| RegistryError::new("recovery_member_family", cause))?,
        );
        Ok(())
    }
    /// Open/verify the exact member on the admitted Root finite worker, then bind before resume.
    /// Keep this owner in durable source state through callback/cancellation; failed partial
    /// duplicates remain in its retained family's checked-close slot and prevent more effects.
    /// # Errors
    /// Retains this identical permit/family and original cause on every native/source failure.
    pub fn bind(
        &mut self,
        expected: RecoveryProcessIdentity,
        context: &JobContext,
    ) -> Result<RecoveryEnrolledDesktop, RegistryError> {
        let result = (|| {
            if self.failed
                || context.owner_id()
                    != self
                        .permit
                        .as_ref()
                        .ok_or_else(|| RegistryError::invalid("recovery_member_watch_consumed"))?
                        .owner_id()
                || context.spec().class != JobClass::InfrastructureLifecycle
                || context.spec().operation != Operation::NativeLifecycle
                || context.deadline() != Some(self.grant.original_budget().until())
                || context.cancellation_requested()
                || context.now() >= self.grant.original_budget().until()
            {
                return Err(RegistryError::invalid("recovery_member_original_control"));
            }
            self.grant
                .check_context(context)
                .map_err(|cause| RegistryError::new("recovery_member_grant", cause))?;
            let family = self
                .family
                .as_ref()
                .ok_or_else(|| RegistryError::invalid("recovery_member_family_unprepared"))?;
            let input = family.duplicate_member(
                Identity { pid: expected.pid(), birth: expected.birth() },
                context,
            )?;
            let permit = self
                .permit
                .take()
                .ok_or_else(|| RegistryError::invalid("recovery_member_watch_consumed"))?;
            let watch = creation::bind(permit, input)
                .ok_or_else(|| RegistryError::invalid("recovery_member_watch_binding"))?;
            let family = self
                .family
                .take()
                .ok_or_else(|| RegistryError::invalid("recovery_member_family_unprepared"))?;
            Ok(RecoveryEnrolledDesktop {
                family,
                watch,
                grant: self.grant.clone(),
                actual: expected,
                terminal: None,
                lifecycle_enrolled: false,
                terminal_verified: false,
                terminal_rejection: None,
                failure: None,
            })
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
}

impl RecoveryEnrolledDesktop {
    /// Return exact matching identity of the independently retained UI process.
    pub const fn actual(&self) -> RecoveryProcessIdentity {
        self.actual
    }
    /// Return the original real registry family, not an unrelated UI process entry.
    pub const fn process(&self) -> sugarsubstitute_contracts::lifecycle::ProcessKey {
        self.family.ticket.process
    }
    /// Return exact grant facts while retaining its genuine same-Root source.
    pub fn key(&self) -> RecoveryGrantKey {
        self.grant.key()
    }
    /// Recheck genuine Root grant/family custody before forwarding an owned lifecycle action.
    /// # Errors
    /// Rejects original Root/source loss, family closure or already observed UI terminal.
    pub fn check(&self) -> Result<(), RegistryError> {
        self.grant.check().map_err(|cause| RegistryError::new("recovery_member_grant", cause))?;
        if !self.family.is_live() || self.terminal.is_some() || self.failure.is_some() {
            return Err(RegistryError::invalid("recovery_member_stopped"));
        }
        Ok(())
    }
    /// Bind Root's actual direct UI pipe to this same independently held process.
    /// Only the successful checked native transfer enables DesktopEnrolled/resume.
    /// Original direct pipe WriteIssued/Written/revocation remain unchanged.
    /// # Errors
    /// Preserves the first physical/source failure and rejected enrollment/handle custody;
    /// a failed owner cannot perform more path/process enrollment effects.
    pub fn bind_lifecycle(
        &mut self,
        input: &mut Option<
            sugarsubstitute_execution::windows_lifecycle_pipe::LifecycleDesktopEnrollment,
        >,
        context: &JobContext,
    ) -> Result<(), RegistryError> {
        let result = (|| {
            self.check()?;
            if self.lifecycle_enrolled
                || context.deadline() != Some(self.grant.original_budget().until())
            {
                return Err(RegistryError::invalid("recovery_lifecycle_original_control"));
            }
            self.family.enroll_member(
                Identity { pid: self.actual.pid(), birth: self.actual.birth() },
                self.grant.key().run(),
                input,
                context,
            )?;
            self.lifecycle_enrolled = true;
            Ok(())
        })();
        if let Err(cause) = &result
            && self.failure.is_none()
        {
            self.failure = Some(cause.clone());
        }
        result
    }
    /// Observe complete physical direct-channel enrollment before the Root replies.
    pub const fn lifecycle_enrolled(&self) -> bool {
        self.lifecycle_enrolled && self.failure.is_none()
    }
    /// Borrow the exact retained first enrollment failure without converting or losing its causes.
    pub const fn failure(&self) -> Option<&RegistryError> {
        self.failure.as_ref()
    }
    /// Observe and validate this UI's actual terminal independently of carrier/handler.
    /// Every original completion/cause stays owned even if its identity is inconsistent.
    /// Cancellation/timeout/owner failure stops forwarding but cannot prove UI exit.
    /// # Errors
    /// Retains wrong completion/physical identity or the exact original native watch cause.
    pub fn observe_terminal(&mut self) -> Result<bool, RegistryError> {
        if let Some(cause) = &self.terminal_rejection {
            return Err(cause.clone());
        }
        if self.terminal.is_none() {
            self.terminal = self.watch.try_take_completion();
        }
        let Some(completed) = &self.terminal else {
            return Ok(false);
        };
        let checked = if completed.scope_id != self.watch.scope_id()
            || completed.spec != self.watch.spec()
        {
            Err(RegistryError::invalid("recovery_member_watch_terminal"))
        } else {
            match &completed.outcome {
                JobOutcome::Success(exit) => {
                    validate_exit(*exit, self.family.ticket.process, self.actual)
                }
                JobOutcome::Failure(cause) => {
                    Err(RegistryError::native_observation("recovery_member_watch", cause.clone()))
                }
                JobOutcome::Cancelled | JobOutcome::TimedOut => Ok(()),
                JobOutcome::Panicked(_) | JobOutcome::Coalesced { .. } => {
                    Err(RegistryError::invalid("recovery_member_watch_nonexit"))
                }
            }
        };
        match checked {
            Ok(()) => {
                self.terminal_verified = true;
                Ok(true)
            }
            Err(cause) => {
                self.terminal_rejection = Some(cause.clone());
                Err(cause)
            }
        }
    }
    /// Observe actual native watcher retirement independently of success or physical UI exit.
    /// A mismatched terminal remains owned and cannot establish observer settlement.
    pub fn observation_retired(&self) -> bool {
        self.terminal.as_ref().is_some_and(|completed| {
            completed.scope_id == self.watch.scope_id() && completed.spec == self.watch.spec()
        })
    }
    /// Return actual validated UI exit independently of prior enrollment/source failures.
    /// Observer cancellation, timeout and retained errors cannot manufacture an exit code.
    pub fn actual_exit(&self) -> Option<ProcessExit> {
        if !self.terminal_verified {
            return None;
        }
        match &self.terminal.as_ref()?.outcome {
            JobOutcome::Success(exit) => Some(*exit),
            _ => None,
        }
    }
    /// Borrow a separate terminal failure without overwriting the original enrollment cause.
    pub const fn watch_failure(&self) -> Option<&RegistryError> {
        self.terminal_rejection.as_ref()
    }
    /// Borrow the exact observed result; a terminal outcome does not establish family cleanup.
    pub const fn terminal(&self) -> Option<&JobCompletion<ProcessExit, NativeObservationError>> {
        self.terminal.as_ref()
    }
    /// Request cancellation of this observer only; never terminate attached/unrelated processes.
    pub fn cancel_watch(&self) {
        self.watch.cancel();
    }
}
/// Physical process facts must match the independently enrolled exact family/member.
fn validate_exit(
    exit: ProcessExit,
    process: sugarsubstitute_contracts::lifecycle::ProcessKey,
    actual: RecoveryProcessIdentity,
) -> Result<(), RegistryError> {
    if exit.correlation.get() != process.id.get()
        || exit.process_id != actual.pid()
        || exit.creation_time_100ns != actual.birth()
    {
        Err(RegistryError::invalid("recovery_member_exit_identity"))
    } else {
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_ui_exit_requires_exact_independent_member_identity_and_preserves_terminal259()
    -> Result<(), Box<dyn std::error::Error>> {
        use sugarsubstitute_contracts::lifecycle::*;
        let owner = CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        };
        let process = ProcessKey { owner, id: ProcessId::new(3)? };
        let actual = RecoveryProcessIdentity::new(4, 5)?;
        let exit = ProcessExit {
            correlation: sugarsubstitute_execution::windows::NativeCorrelation::new(3)?,
            process_id: 4,
            creation_time_100ns: 5,
            exit_code: 259,
        };
        validate_exit(exit, process, actual)?;
        for changed in [
            ProcessExit { process_id: 6, ..exit },
            ProcessExit { creation_time_100ns: 6, ..exit },
            ProcessExit {
                correlation: sugarsubstitute_execution::windows::NativeCorrelation::new(6)?,
                ..exit
            },
        ] {
            assert!(validate_exit(changed, process, actual).is_err());
        }
        assert_eq!(exit.exit_code, 259);
        Ok(())
    }
}
