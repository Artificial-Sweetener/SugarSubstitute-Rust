// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own entry admission policy through retained typed execution lifetimes.
use super::*;
impl CoordinatorEntryOwner {
    /// Admit actual namespace and installed running-image discovery once.
    /// # Errors
    /// Rejects closed/busy state, foreign execution, invalid setup or actual admission.
    pub fn submit_discovery(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
    ) -> Result<JobId, EntryAdmissionError> {
        if self.phase != EntryPhase::Inert {
            return Err(EntryAdmissionError::Busy);
        }
        self.validate(client, scope, spec, false)?;
        let shared = self.lifetime.clone();
        let port = self.port.clone();
        let paths = self.paths.as_ref().ok_or(EntryAdmissionError::Closed)?.clone();
        let job = client
            .submit_blocking(scope, spec, move |context| {
                if !shared.open.load(Ordering::Acquire) {
                    return Err(ClaimError::invalid("entry_discovery_closed"));
                }
                let mut state = lock(&shared.state);
                if state.memory.is_some() || paths.preparation_memory.get().is_some() {
                    return Err(ClaimError::invalid("entry_repeated_preclaim"));
                }
                let memory = Arc::new(
                    context
                        .memory()
                        .claim(ENTRY_PREPARATION_BYTES)
                        .map_err(|cause| ClaimError::new("entry_namespace_capacity", cause))?,
                );
                paths
                    .preparation_memory
                    .set(memory.clone())
                    .map_err(|_| ClaimError::invalid("entry_repeated_preclaim"))?;
                state.memory = Some(memory);
                port.discover(&mut state, &paths.installation, &paths.executable, &context)
                    .map(Actual::Discovered)
            })
            .map_err(EntryAdmissionError::Execution)?;
        self.accept(client, scope, job, EntryPhase::Discovering);
        Ok(spec.id)
    }
    /// Admit actual stable exclusion acquisition and checked source observation once.
    /// # Errors
    /// Rejects unresolved discovery, closed/foreign state or invalid original budget.
    pub fn submit_election(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
    ) -> Result<JobId, EntryAdmissionError> {
        if self.phase != EntryPhase::Discovered {
            return Err(EntryAdmissionError::Busy);
        }
        self.validate(client, scope, spec, false)?;
        let shared = self.lifetime.clone();
        let port = self.port.clone();
        let job = client
            .submit_blocking(scope, spec, move |context| {
                if !shared.open.load(Ordering::Acquire) {
                    return Err(ClaimError::invalid("entry_election_closed"));
                }
                port.elect(&mut lock(&shared.state), &context).map(Actual::Elected)
            })
            .map_err(EntryAdmissionError::Execution)?;
        self.accept(client, scope, job, EntryPhase::Electing);
        Ok(spec.id)
    }
    /// Admit checked retirement under the first original cleanup deadline.
    /// # Errors
    /// Rejects outstanding work, foreign execution, deadline renewal or actual admission.
    pub fn submit_close(
        &mut self,
        client: &ExecutionClient,
        cleanup: &Scope,
        spec: JobSpec,
    ) -> Result<JobId, EntryAdmissionError> {
        self.close_admission();
        if self.phase == EntryPhase::Settled {
            return Err(EntryAdmissionError::Closed);
        }
        let until = spec.deadline.ok_or(EntryAdmissionError::Specification)?;
        if self.close_deadline.is_some_and(|old| until > old) {
            return Err(EntryAdmissionError::Specification);
        }
        self.close_deadline = Some(self.close_deadline.map_or(until, |old| old.min(until)));
        self.validate(client, cleanup, spec, true)?;
        let shared = self.lifetime.clone();
        let port = self.port.clone();
        let job = client
            .submit_blocking(cleanup, spec, move |context| {
                let mut state = lock(&shared.state);
                if state.borrowed {
                    return Err(ClaimError::invalid("entry_namespace_borrowed"));
                }
                port.close(&mut state, &context)?;
                if !state.retired
                    || state.election.is_some()
                    || !state.handles.is_empty()
                    || state.memory.is_some()
                {
                    return Err(ClaimError::invalid("entry_close_unsettled"));
                }
                shared.retired.store(true, Ordering::Release);
                Ok(Actual::Closed)
            })
            .map_err(EntryAdmissionError::Execution)?;
        self.accept(client, cleanup, job, EntryPhase::Closing);
        Ok(spec.id)
    }
    fn validate(
        &self,
        client: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        cleanup: bool,
    ) -> Result<(), EntryAdmissionError> {
        if self.pending.is_some() {
            return Err(EntryAdmissionError::Busy);
        }
        if self.events.iter().filter(|entry| entry.is_some()).count() > 2 {
            return Err(EntryAdmissionError::Busy);
        }
        if self.execution.is_some_and(|owner| owner != scope.owner_id()) {
            return Err(EntryAdmissionError::Identity);
        }
        let now = client.now().map_err(EntryAdmissionError::Clock)?;
        let until = spec.deadline.ok_or(EntryAdmissionError::Specification)?;
        if spec.class != JobClass::InfrastructureLifecycle
            || spec.operation != Operation::NativeLifecycle
            || spec.replacement.is_some()
            || spec.reservation_bytes != ENTRY_PREPARATION_BYTES
            || now >= until
        {
            return Err(EntryAdmissionError::Specification);
        }
        if !cleanup
            && (!self.lifetime.open.load(Ordering::Acquire)
                || now < self.budget.started()
                || until > self.budget.until()
                || now >= self.budget.until())
        {
            return Err(EntryAdmissionError::Closed);
        }
        Ok(())
    }
    fn accept(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        job: OwnedJob<Actual, ClaimError>,
        phase: EntryPhase,
    ) {
        self.execution = Some(scope.owner_id());
        self.client = Some(client.clone());
        self.pending = Some(Pending { job, cleanup: phase == EntryPhase::Closing });
        self.phase = phase;
    }
}
