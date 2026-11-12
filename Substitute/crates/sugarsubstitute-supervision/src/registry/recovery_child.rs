// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep finite suspended UI creation and its independent real exit observation owned.
mod driver;
mod input;
mod observation;
use super::*;
pub use input::{InheritedRecoveryJobInput, RecoveryChildInput, RecoveryChildInputRejection};
use sugarsubstitute_contracts::{
    execution::{JobCompletion, JobOutcome, Moment},
    recovery_supervisor::RecoveryProcessIdentity,
};
use sugarsubstitute_execution::{
    process_creation::{
        ProcessCreationFailure, ProcessCreationMemory, ProcessCreationReport, ProcessCreationWake,
    },
    recovery_supervisor::{AuthenticatedRecoveryOrigin, RecoveryDesktopEnrollment},
    windows_lifecycle_pipe::{CapturedLifecycleStart, RecoveryDesktopBootstrap},
};
const TRANSIENT_BYTES: u64 = 1024 * 1024;
const CREATION_BYTES: u64 = 4 * 1024 * 1024;
type Factory = Box<
    dyn FnOnce(&RecoveryDesktopBootstrap, &JobContext) -> Result<WindowsLaunchInput, RegistryError>
        + Send,
>;
struct State {
    pending_job: Option<usize>,
    attempted: bool,
    created: Option<RecoveryProcessIdentity>,
    resumed: bool,
    native_retired: bool,
    process_absent_proven: bool,
    enrollment: Option<RecoveryDesktopEnrollment>,
    wake: Option<ProcessCreationWake>,
    permit: Option<ProcessWatchPermit>,
    watch: Option<WatchJob>,
    memory: Option<ProcessCreationMemory>,
    cleanup_memory: Option<sugarsubstitute_contracts::execution::MemoryClaim>,
}
impl State {
    /// Retain the one original lexical slot before any source or native admission.
    fn unstarted(pending_job: Option<usize>) -> Self {
        Self {
            pending_job,
            attempted: false,
            created: None,
            resumed: false,
            native_retired: false,
            process_absent_proven: false,
            enrollment: None,
            wake: None,
            permit: None,
            watch: None,
            memory: None,
            cleanup_memory: None,
        }
    }
}
struct Shared {
    state: Mutex<State>,
}
type Creation = OwnedJob<ProcessCreationReport, ProcessCreationFailure>;
/// Keep the original creation outcome, passive observations and independent UI watch alive.
/// The Root/helper own profile containment. This owner closes only its own native duplicates.
#[must_use = "Observe creation, UI watch and checked local output retirement"]
pub struct RecoveryChildCreation {
    shared: Arc<Shared>,
    origin: AuthenticatedRecoveryOrigin,
    watch_completion: Option<JobCompletion<ProcessExit, NativeObservationError>>,
    creation_completion: Option<JobCompletion<ProcessCreationReport, ProcessCreationFailure>>,
    creation: Option<Creation>,
    cleanup: Option<OwnedJob<(), RegistryError>>,
    cleanup_completion: Option<JobCompletion<(), RegistryError>>,
    exit_verified: bool,
    watch_admitted: bool,
    cleanup_until: Option<Moment>,
    watch_id: (sugarsubstitute_contracts::execution::ScopeId, JobSpec),
}
/// Preserve the original reserved observer and inherited slot after creation admission rejects.
pub struct RecoveryChildCreationRejection {
    /// Original execution/source/specification rejection.
    pub cause: RegistryAdmissionError,
    /// Actual close/terminal observation owner; caller input remains unchanged.
    pub owner: RecoveryChildCreation,
}
/// Current independently observed physical child lifecycle facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryChildStatus {
    /// Actual suspended UI PID and kernel birth, once created.
    pub created: Option<RecoveryProcessIdentity>,
    /// The exact independently enrolled primary thread was actually resumed.
    pub resumed: bool,
    /// Actual watch exit code, including259 when the native watch signaled.
    pub exit: Option<u32>,
    /// Original creation completed and all local owned outputs checked-retired.
    pub creation_settled: bool,
    /// Actual original creation terminal; cancellation never stands for resumed UI.
    pub creation_terminal: Option<sugarsubstitute_contracts::execution::TerminalKind>,
    /// Actual observer terminal; only successful exact process exit populates `exit`.
    pub watch_terminal: Option<sugarsubstitute_contracts::execution::TerminalKind>,
}
impl RecoveryChildCreation {
    /// Admit independent observation before the single actual finite creation factory.
    /// Input remains identical on rejection. Rejected watch owners are returned for observation.
    /// # Errors
    /// Rejects original source/control/class/deadline, watch or real queue/memory capacity.
    #[allow(clippy::too_many_arguments)]
    #[expect(
        clippy::result_large_err,
        reason = "Preserve exact reserved native observer on rejection"
    )]
    pub fn submit(
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        native_scope: &Scope,
        watch: JobSpec,
        origin: &AuthenticatedRecoveryOrigin,
        input: &mut Option<RecoveryChildInput>,
    ) -> Result<Self, RecoveryChildCreationRejection> {
        let shared = input
            .as_ref()
            .map(|input| input.shared.clone())
            .unwrap_or_else(|| Arc::new(Shared { state: Mutex::new(State::unstarted(None)) }));
        let pending = lock(&shared.state).pending_job;
        let mut owner = Self {
            shared: shared.clone(),
            origin: origin.clone(),
            watch_completion: None,
            creation_completion: None,
            creation: None,
            cleanup: None,
            cleanup_completion: None,
            exit_verified: false,
            watch_admitted: false,
            cleanup_until: None,
            watch_id: (native_scope.id(), watch),
        };
        let decision = (|| {
            origin.check().map_err(RegistryAdmissionError::Execution)?;
            submission::validate_blocking(spec)?;
            submission::validate_watch(watch)?;
            if pending.is_none()
                || spec.reservation_bytes != CREATION_BYTES
                || spec.deadline != Some(origin.original_until())
                || native_scope.spec().deadline.is_some()
                || watch.id >= spec.id
            {
                return Err(RegistryAdmissionError::InvalidSpecification);
            }
            let correlation = sugarsubstitute_execution::windows::NativeCorrelation::new(
                origin.key().run().instance.get(),
            )
            .map_err(|_| RegistryAdmissionError::InvalidSpecification)?;
            {
                let mut state = lock(&shared.state);
                if state.attempted {
                    return Err(RegistryAdmissionError::Closed);
                }
                state.attempted = true;
            }
            let permit = execution
                .reserve_process_watch(native_scope, watch, correlation)
                .map_err(RegistryAdmissionError::Execution)?;
            lock(&shared.state).permit = Some(permit);
            owner.watch_admitted = true;
            let origin = origin.clone();
            let mut original = input.take();
            let result = execution.submit_recovery_desktop_creation(
                scope,
                spec,
                &owner.origin,
                &mut original,
                move |input, context, memory| {
                    driver::Driver::construct(shared, input, origin, context, memory)
                },
            );
            *input = original;
            match result {
                Ok(job) => owner.creation = Some(job),
                Err(cause) => return Err(RegistryAdmissionError::Execution(cause)),
            }
            Ok(())
        })();
        match decision {
            Ok(()) => Ok(owner),
            Err(cause) => {
                let permit = { lock(&owner.shared.state).permit.take() };
                if let Some(permit) = permit {
                    lock(&owner.shared.state).watch = Some(permit.cancel_unbound());
                }
                Err(RecoveryChildCreationRejection { cause, owner })
            }
        }
    }
    /// Borrow passive exact created identity without interpreting carrier statements as UI exit.
    pub fn created(&self) -> Option<RecoveryProcessIdentity> {
        lock(&self.shared.state).created
    }
    /// Deliver the actual source-bound Root enrollment once; wake only the admitted creation.
    /// # Errors
    /// Retains a rejected token in the caller and forbids changed UI/source or repeated enrollment.
    pub fn enroll(
        &self,
        input: &mut Option<RecoveryDesktopEnrollment>,
    ) -> Result<(), RegistryError> {
        let (actual, memory) = {
            let state = lock(&self.shared.state);
            if state.resumed || state.enrollment.is_some() {
                return Err(RegistryError::invalid("recovery_child_enrollment_repeat"));
            }
            (
                state
                    .created
                    .ok_or_else(|| RegistryError::invalid("recovery_child_not_created"))?,
                state
                    .memory
                    .as_ref()
                    .ok_or_else(|| RegistryError::invalid("recovery_child_memory"))?
                    .clone(),
            )
        };
        input
            .as_ref()
            .ok_or_else(|| RegistryError::invalid("recovery_child_enrollment_missing"))?
            .check(&self.origin, actual, &memory)
            .map_err(|cause| RegistryError::new("recovery_child_enrollment", cause))?;
        let wake = {
            let mut state = lock(&self.shared.state);
            if state.created != Some(actual) || state.resumed || state.enrollment.is_some() {
                return Err(RegistryError::invalid("recovery_child_enrollment_changed"));
            }
            state.enrollment = input.take();
            state.wake.clone()
        };
        if let Some(wake) = wake {
            wake.observed();
        }
        Ok(())
    }
    /// Request creation cancellation without asserting UI exit, native close or watch retirement.
    pub fn cancel_creation(&self) {
        if let Some(job) = &self.creation {
            job.cancel();
        }
    }
    /// Cancel only the independent observer; its original terminal still must be consumed.
    pub fn cancel_watch(&self) {
        let mut state = lock(&self.shared.state);
        if let Some(permit) = state.permit.take() {
            state.watch = Some(permit.cancel_unbound());
        }
        if let Some(watch) = &state.watch {
            watch.cancel();
        }
    }
    /// Observe whether an actual inherited input still needs its original close-only cleanup.
    pub fn unstarted_cleanup_required(&self) -> bool {
        let state = lock(&self.shared.state);
        state.pending_job.is_some()
            && (self.creation.is_none() || self.creation_completion.is_some())
    }
    /// Close an inherited input whose cancelled/rejected factory never adopted it.
    /// The caller supplies its already-latched shutdown cutoff; this admits no creation.
    /// # Errors
    /// Rejects an active original factory, repeat cleanup, wrong finite job or actual admission.
    pub fn submit_unstarted_cleanup(
        &mut self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
    ) -> Result<(), RegistryAdmissionError> {
        submission::validate_blocking(spec)?;
        if self.cleanup.is_some()
            || self.creation.is_some() && self.creation_completion.is_none()
            || spec.deadline.is_none()
        {
            return Err(RegistryAdmissionError::Busy);
        }
        if self.cleanup_until.is_some_and(|until| Some(until) != spec.deadline) {
            return Err(RegistryAdmissionError::InvalidSpecification);
        }
        self.cleanup_until = spec.deadline;
        let shared = self.shared.clone();
        let origin = self.origin.clone();
        self.cleanup = Some(
            execution
                .submit_blocking(scope, spec, move |context| {
                    driver::close_unstarted(&shared, &origin, &context)
                })
                .map_err(RegistryAdmissionError::Execution)?,
        );
        Ok(())
    }
}
impl std::fmt::Debug for RecoveryChildCreationRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecoveryChildCreationRejection { original owners retained }")
    }
}
impl std::fmt::Display for RecoveryChildCreationRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Recovery child creation rejected")
    }
}
impl std::error::Error for RecoveryChildCreationRejection {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cause)
    }
}

impl Drop for RecoveryChildCreation {
    fn drop(&mut self) {
        if self.settled() {
            return;
        }
        self.cancel_creation();
        self.cancel_watch();
        // Outstanding physical slots and actual terminal/claim owners outlive diagnostics.
        std::mem::forget(self.shared.clone());
        std::mem::forget(self.origin.clone());
        std::mem::forget(self.creation.take());
        std::mem::forget(self.creation_completion.take());
        std::mem::forget(self.watch_completion.take());
        std::mem::forget(self.cleanup.take());
        std::mem::forget(self.cleanup_completion.take());
        tracing::error!(
            operation = "recovery_child_uncertain_drop",
            "Unresolved original child inputs, observations and admitted claims retained"
        );
    }
}
