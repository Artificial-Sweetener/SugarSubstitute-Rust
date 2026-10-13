// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admission precedes lease creation, module registration and process effects.
use super::*;
use sugarsubstitute_contracts::execution::{ExecutionPort, JobClass, JobSpec, Operation, Scope};
use sugarsubstitute_execution::ExecutionClient;

fn validate(spec: JobSpec, bytes: usize) -> Result<(), CrashAdmissionError> {
    if spec.operation != Operation::NativeLifecycle
        || spec.class != JobClass::InfrastructureLifecycle
        || spec.replacement.is_some()
        || spec.deadline.is_none()
        || spec.reservation_bytes < bytes as u64
    {
        return Err(CrashAdmissionError::InvalidSpecification);
    }
    Ok(())
}
impl WindowsCrashCapture {
    /// Admit finite start with the caller's original deadline and reserved watch.
    ///
    /// # Errors
    /// Returns every rejection with its original input/watch, before native effects.
    /// Accepted late cancellation retains partial native effects independently of
    /// the discarded unit result. `Registered` is emitted only after actual
    /// native registration, kernel identity and watch binding have been observed.
    pub fn submit_start(
        &mut self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        budget: CaptureBudget,
        watch: ProcessWatchPermit,
        input: CaptureStartInput,
    ) -> Result<(), StartRejected> {
        let rejection = if self.shared.is_some() || self.snapshot.state != CaptureState::Dormant {
            Some(CrashAdmissionError::Busy)
        } else if watch.owner_id() != scope.owner_id()
            || watch.deadline().is_some()
            || self.execution_id.is_some_and(|original| original != scope.owner_id())
            || self.start_budget.is_some_and(|original| original != budget)
            || spec
                .deadline
                .is_none_or(|deadline| deadline <= budget.started() || deadline > budget.until())
        {
            Some(CrashAdmissionError::InvalidSpecification)
        } else {
            match execution.now() {
                Ok(now)
                    if now < budget.started()
                        || spec.deadline.is_some_and(|deadline| deadline <= now) =>
                {
                    Some(CrashAdmissionError::InvalidSpecification)
                }
                Ok(_) => validate(spec, input.required_reservation_bytes()).err(),
                Err(cause) => Some(CrashAdmissionError::Execution(
                    sugarsubstitute_contracts::execution::AdmissionError::Unexpected(cause),
                )),
            }
        };
        let shared = Arc::new(Shared {
            attempts: Mutex::new(None),
            owner: self.owner,
            build: Mutex::new(None),
            execution: execution.clone(),
            execution_id: scope.owner_id(),
            native: Mutex::new(NativeState {
                lease: None,
                borrowed: false,
                watch: None,
                registered: false,
                handler: None,
                creator: None,
                retired: false,
                deadline: None,
                close_deadline: None,
                errors: std::array::from_fn(|_| None),
                omitted: 0,
            }),
            input: Mutex::new(Some(input)),
            permit: Mutex::new(Some(watch)),
            port: self.port.clone(),
            retained_memory: Mutex::new(None),
        });
        if let Some(reason) = rejection {
            return Err(StartRejected { reason, pending: shared });
        }
        self.start_budget = Some(budget);
        self.execution = Some(execution.clone());
        self.execution_id = Some(scope.owner_id());
        let task = shared.clone();
        let owner = self.owner;
        match execution
            .submit_blocking(scope, spec, move |context| work::start(task, owner, context))
        {
            Ok(job) => {
                self.shared = Some(shared);
                self.start = Some(job);
                self.snapshot.state = CaptureState::Starting;
                self.execution = Some(execution.clone());
                self.execution_id = Some(scope.owner_id());
                Ok(())
            }
            Err(reason) => Err(StartRejected {
                reason: CrashAdmissionError::Execution(reason),
                pending: shared,
            }),
        }
    }
    /// Admit one finite stop/retirement attempt after caller capture policy permits it.
    ///
    /// This request does not prove a dump, handler exit or M1 family settlement.
    /// Incomplete I/O/exit keeps `Closing`; further finite attempts retain the
    /// original caller deadline. No module unload or registration retry occurs.
    ///
    /// # Errors
    /// Rejects active start/close, dormant/closed owners and invalid or rejected work.
    pub fn submit_close(
        &mut self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
    ) -> Result<(), CrashAdmissionError> {
        validate(spec, 4096)?;
        if self.execution_id != Some(scope.owner_id()) {
            return Err(CrashAdmissionError::InvalidSpecification);
        }
        if self
            .close_deadline
            .is_some_and(|original| spec.deadline.is_none_or(|deadline| deadline > original))
        {
            return Err(CrashAdmissionError::InvalidSpecification);
        }
        if self.start.is_some()
            || self.close.is_some()
            || matches!(self.snapshot.state, CaptureState::Dormant | CaptureState::Closed)
        {
            return Err(CrashAdmissionError::Busy);
        }
        let shared = self.shared.as_ref().ok_or(CrashAdmissionError::Busy)?.clone();
        if let Some(attempts) = lock(&shared.attempts).as_mut() {
            attempts.close();
        }
        let deadline = spec.deadline.ok_or(CrashAdmissionError::InvalidSpecification)?;
        self.close_deadline = Some(self.close_deadline.map_or(deadline, |old| old.min(deadline)));
        let job = execution
            .submit_blocking(scope, spec, move |context| work::close(shared, context))
            .map_err(CrashAdmissionError::Execution)?;
        self.close = Some(job);
        self.snapshot.state = CaptureState::Closing;
        Ok(())
    }
}
