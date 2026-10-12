// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep partial profile effects outside disposable execution result payloads.
use super::*;
use sugarsubstitute_contracts::execution::{ExecutionPort, JobClass, JobContext, Operation};
impl WindowsProfileLease {
    /// Admit one acquisition attempt, retaining partial outputs independently of cancellation.
    /// # Errors
    /// Rejects a repeated attempt, foreign execution, invalid finite job or admission overload.
    pub fn submit_acquire(
        &mut self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
    ) -> Result<JobId, ClaimAdmissionError> {
        if self.acquire_attempted {
            return Err(ClaimAdmissionError::Closed);
        }
        self.validate_submission(scope, spec)?;
        let path = self.path.clone();
        let run = self.run;
        let port = self.port.clone();
        self.submit(
            execution,
            scope,
            spec,
            move |state, context| port.acquire(state, &path, run, context),
            false,
        )?;
        self.acquire_attempted = true;
        Ok(spec.id)
    }
    /// Match the actual WebView2 environment folder by account security and native file identity.
    /// The bounded UTF16 observation is copied once under the retained profile allocation.
    /// # Errors
    /// Rejects malformed, repeated or unheld validation, foreign execution or admission failure.
    pub fn submit_environment_validation(
        &mut self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        identity: MainIdentity,
        observed: &[u16],
    ) -> Result<JobId, ClaimAdmissionError> {
        self.validate_submission(scope, spec)?;
        if !self.is_held() || observed.is_empty() || observed.len() > 32766 || observed.contains(&0)
        {
            return Err(ClaimAdmissionError::Specification);
        }
        let binding = self.attempts.admit(
            identity,
            spec.deadline.ok_or(ClaimAdmissionError::Specification)?,
            attempt::ProofKind::Environment,
        )?;
        let mut owned = Vec::new();
        owned.try_reserve_exact(observed.len()).map_err(|e| {
            ClaimAdmissionError::Native(ClaimError::new("profile_environment_buffer", e))
        })?;
        owned.extend_from_slice(observed);
        if owned.capacity() > 32766 {
            return Err(ClaimAdmissionError::Specification);
        }
        let port = self.port.clone();
        self.submit(
            execution,
            scope,
            spec,
            move |state, context| {
                port.validate_environment(state, &owned, context)?;
                state.environment = Some((binding, context.job_id()));
                Ok(())
            },
            false,
        )?;
        self.pending_attempt = Some(identity);
        Ok(spec.id)
    }
    /// Qualify the actual controller browser PID in the exact retained desktop Job.
    /// A failed/unadmitted proof leaves browser ownership uncertain and blocks release.
    /// # Errors
    /// Rejects foreign registry/execution, invalid jobs, absent desktop family or repeated proof.
    pub fn submit_browser_validation(
        &mut self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        registry: &WindowsProcessRegistry,
        identity: MainIdentity,
        browser_process_id: u32,
    ) -> Result<JobId, ClaimAdmissionError> {
        if !self.is_held() {
            return Err(ClaimAdmissionError::Busy);
        }
        let binding = self.attempts.admit(
            identity,
            spec.deadline.ok_or(ClaimAdmissionError::Specification)?,
            attempt::ProofKind::Browser,
        )?;
        // An observed controller is uncertain until this exact retained Job proves it.
        self.shared.active.store(UNCERTAIN, Ordering::Release);
        self.validate_submission(scope, spec)?;
        self.registry_matches(registry)?;
        if browser_process_id == 0 {
            return Err(ClaimAdmissionError::Specification);
        }
        let LaunchState::Accepted(ticket) = self.launch else {
            return Err(ClaimAdmissionError::Busy);
        };
        let family = match registry.profile_family(ticket, self.run) {
            Ok(family) => family,
            Err(cause) => {
                self.shared.active.store(UNCERTAIN, Ordering::Release);
                return Err(ClaimAdmissionError::Registry(cause));
            }
        };
        if !family.identity.same_as(&self.registry) || family.ticket != ticket {
            self.shared.active.store(UNCERTAIN, Ordering::Release);
            return Err(ClaimAdmissionError::Identity);
        }
        let result = self.submit(
            execution,
            scope,
            spec,
            move |state, context| {
                let identity = family
                    .qualify_browser(browser_process_id, context)
                    .map_err(|e| ClaimError::new("profile_browser_family", e))?;
                state.browser = Some((binding, context.job_id(), identity, family));
                Ok(())
            },
            true,
        );
        if result.is_err() {
            self.shared.active.store(UNCERTAIN, Ordering::Release);
        } else {
            self.pending_attempt = Some(identity);
            self.pending_browser = true;
        }
        result
    }
    /// Release only after exact accepted family settlement or precise rejected watch retirement.
    /// Keep an accepted settled registry record until this operation is consumed.
    /// # Errors
    /// Uncertain browser ownership, an outstanding capsule, a live family or foreign identity prevents release.
    pub fn submit_release(
        &mut self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        registry: &WindowsProcessRegistry,
    ) -> Result<JobId, ClaimAdmissionError> {
        self.validate_submission(scope, spec)?;
        self.registry_matches(registry)?;
        if self.shared.active.load(Ordering::Acquire) == UNCERTAIN
            || self.prepared.load(Ordering::Acquire) == 1
        {
            return Err(ClaimAdmissionError::Busy);
        }
        let retained =
            registry.retained_desktop(self.run).map_err(ClaimAdmissionError::Registry)?;
        match self.launch {
            LaunchState::Unattempted | LaunchState::Rejected if retained.is_none() => (),
            LaunchState::Accepted(ticket)
                if retained.as_ref().is_some_and(|snapshot| {
                    snapshot.process == ticket.process() && snapshot.settled
                }) => {}
            _ => return Err(ClaimAdmissionError::Busy),
        }
        let port = self.port.clone();
        let shared = self.shared.clone();
        let deadline = spec.deadline.ok_or(ClaimAdmissionError::Specification)?;
        if self.release_deadline.is_some_and(|old| deadline > old) {
            return Err(ClaimAdmissionError::Specification);
        }
        self.release_deadline =
            Some(self.release_deadline.map_or(deadline, |old| old.min(deadline)));
        self.shared.closing.store(true, Ordering::Release);
        self.attempts.invalidate();
        let admitted = self.submit(
            execution,
            scope,
            spec,
            move |state, context| {
                let deadline = context
                    .deadline()
                    .ok_or_else(|| ClaimError::invalid("profile_release_deadline"))?;
                if state.close_deadline.is_some_and(|old| deadline > old) {
                    return Err(ClaimError::invalid("profile_renewed_release_deadline"));
                }
                state.close_deadline =
                    Some(state.close_deadline.map_or(deadline, |old| old.min(deadline)));
                port.release(state, context)?;
                shared.active.store(RELEASED, Ordering::Release);
                Ok(())
            },
            false,
        );
        admitted?;
        Ok(spec.id)
    }
    pub(super) fn is_held(&self) -> bool {
        self.shared.active.load(Ordering::Acquire) == HELD
            && !self.shared.closing.load(Ordering::Acquire)
    }
    pub(super) fn validate_submission(
        &self,
        scope: &Scope,
        spec: JobSpec,
    ) -> Result<(), ClaimAdmissionError> {
        if self.pending.is_some() {
            return Err(ClaimAdmissionError::Busy);
        }
        if self.execution.is_some_and(|old| old != scope.owner_id()) {
            return Err(ClaimAdmissionError::Identity);
        }
        if spec.operation != Operation::NativeLifecycle
            || spec.class != JobClass::InfrastructureLifecycle
            || spec.deadline.is_none()
            || spec.replacement.is_some()
            || spec.reservation_bytes < PROFILE_OPERATION_BYTES
        {
            return Err(ClaimAdmissionError::Specification);
        }
        Ok(())
    }
    pub(super) fn submit(
        &mut self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        work: impl FnOnce(&mut storage::NativeState, &JobContext) -> Result<(), ClaimError>
        + Send
        + 'static,
        browser: bool,
    ) -> Result<JobId, ClaimAdmissionError> {
        let shared = self.shared.clone();
        let owner = scope.owner_id();
        let job = execution
            .submit_blocking(scope, spec, move |context| {
                let mut native = lock(&shared.native);
                if native.is_none() {
                    *native = Some(Box::new(storage::NativeState::new()));
                }
                let state =
                    native.as_mut().ok_or_else(|| ClaimError::invalid("profile_native_state"))?;
                if context.owner_id() != owner {
                    return Err(ClaimError::invalid("profile_worker_execution"));
                }
                let result = storage::check(&context).and_then(|()| work(state, &context));
                if browser && result.is_ok() && state.browser.is_some() && state.held {
                    shared.active.store(HELD, Ordering::Release);
                }
                shared.observed_held.store(state.held, Ordering::Release);
                if state.held && shared.active.load(Ordering::Acquire) == INITIAL {
                    shared.active.store(HELD, Ordering::Release);
                }
                if let Err(cause) = &result {
                    if browser {
                        shared.active.store(UNCERTAIN, Ordering::Release);
                    }
                    if state.failure.is_none() {
                        state.failure = Some(cause.clone());
                    }
                }
                result
            })
            .map_err(ClaimAdmissionError::Execution)?;
        self.pending = Some(job);
        self.execution = Some(owner);
        Ok(spec.id)
    }
}
