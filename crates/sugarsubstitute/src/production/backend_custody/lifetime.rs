// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the one real execution and custody source through authenticated pumping.
use super::role::Role;
use std::{error::Error, num::NonZeroUsize, sync::Arc, time::Instant};
use sugarsubstitute_contracts::{
    backend_custody::{BACKEND_CUSTODY_FRAME_BYTES, BackendCustodyCommand, BackendCustodyFrame},
    execution::*,
};
use sugarsubstitute_desktop::{HeadlessLifetime, HeadlessStep, StartupPreparationStep};
use sugarsubstitute_execution::{
    ExecutionClient, ExecutionOwner, MonotonicClock, backend_custody::*,
};
use sugarsubstitute_supervision::BackendCustodySource;
type DriverFactory = Box<
    dyn FnOnce(
            &JobContext,
            &BackendCustodyMemory,
        ) -> Result<
            Box<dyn BackendCustodyDriver>,
            sugarsubstitute_execution::windows::NativeObservationError,
        > + Send,
>;
/// All admitted outcomes, source aliases and scopes precede the actual execution owner.
pub(super) struct Lifetime {
    role: Role,
    terminals: super::terminal::Terminals,
    session: Option<BackendCustodyPipeSession>,
    worker: Option<OwnedJob<BackendCustodyWorkerReport, BackendCustodyWorkerFailure>>,
    worker_start: Option<BackendCustodyWorkerStart>,
    rejected_factory: Option<DriverFactory>,
    capture: super::capture::HelperCapture,
    cleanup: super::cleanup::Cleanup,
    capture_scope: Option<Scope>,
    capture_input: Option<sugarsubstitute_execution::InheritedCaptureAttachInput>,
    source: Option<BackendCustodySource>,
    capability: Option<LifecycleClientCapability>,
    scope: Option<Scope>,
    client: Option<ExecutionClient>,
    execution: Option<ExecutionOwner>,
    connected: bool,
    authenticated: bool,
    hello: bool,
    hello_written: bool,
    closing: bool,
    until: Option<Moment>,
    uncaptured_sampled: bool,
}
impl Lifetime {
    /// Retain inert invocation data; no handle or executor is created here.
    pub(super) fn new(role: Role) -> Self {
        Self {
            capture: super::capture::HelperCapture::new(role.key),
            cleanup: super::cleanup::Cleanup::default(),
            role,
            terminals: super::terminal::Terminals::default(),
            session: None,
            worker: None,
            worker_start: None,
            rejected_factory: None,
            capture_scope: None,
            capture_input: None,
            source: None,
            capability: None,
            scope: None,
            client: None,
            execution: None,
            connected: false,
            authenticated: false,
            hello: false,
            hello_written: false,
            closing: false,
            until: None,
            uncaptured_sampled: false,
        }
    }
    /// Drain original completion diagnostics before acknowledging the bounded owner facts.
    fn drain(&mut self, max: NonZeroUsize) -> Result<(), Box<dyn Error>> {
        let batch =
            self.client.as_ref().ok_or("Custody execution missing")?.drain_completions(max)?;
        self.capture.observe(&batch)?;
        Ok(())
    }
    /// Complete actual kernel/capability authentication before creating the funded source.
    fn authenticate(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(session) = &mut self.session else {
            return Ok(());
        };
        if let Some(done) = session.take_terminal() {
            self.terminals.pipe(done, self.closing, self.authenticated, self.role.key)?;
            return Err("Custody transport ended before source adoption".into());
        }
        for fact in session.drain(NonZeroUsize::MAX) {
            match fact {
                BackendCustodyPipeObservation::Prepared => {
                    if session.control().prepared_key() != Some(self.role.key) {
                        return Err("Custody bootstrap identity rejected".into());
                    }
                    self.capability = session.control().take_client_capability()?;
                }
                BackendCustodyPipeObservation::Connected => {
                    if self.connected {
                        return Err("Custody connection repeated".into());
                    }
                    self.connected = true;
                    session.control().verify_capability(
                        *self.capability.as_ref().ok_or("Custody capability absent")?.bytes(),
                        session.control().original_deadline(),
                    )?;
                }
                BackendCustodyPipeObservation::Authenticated(peer) => {
                    if !self.connected
                        || self.authenticated
                        || peer.key() != self.role.key
                        || peer.process().is_some()
                    {
                        return Err("Custody authenticated parent rejected".into());
                    }
                    self.authenticated = true;
                }
                BackendCustodyPipeObservation::WriteIssued { operation, .. }
                    if self.hello && operation.get() == 1 => {}
                BackendCustodyPipeObservation::Written { operation, bytes }
                    if self.hello
                        && operation.get() == 1
                        && bytes as usize == BACKEND_CUSTODY_FRAME_BYTES =>
                {
                    self.hello_written = true
                }
                _ => return Err("Custody initial observation rejected".into()),
            }
        }
        if self.authenticated
            && !self.hello
            && let Some(mut buffer) = session.control().take_write_buffer()?
        {
            let frame = BackendCustodyFrame {
                key: self.role.key,
                nonce: 1,
                command: BackendCustodyCommand::Hello {
                    capability: *self
                        .capability
                        .as_ref()
                        .ok_or("Custody capability absent")?
                        .bytes(),
                },
            }
            .encode()?;
            buffer
                .as_mut_bytes()
                .get_mut(..BACKEND_CUSTODY_FRAME_BYTES)
                .ok_or("Custody Hello capacity")?
                .copy_from_slice(&frame);
            session
                .control()
                .write(
                    LifecyclePipeOperationId::new(1)?,
                    session.control().original_deadline(),
                    buffer,
                    BACKEND_CUSTODY_FRAME_BYTES,
                )
                .map_err(|rejected| rejected.error)?;
            self.hello = true;
            self.capability = None;
        }
        if self.hello_written && self.worker.is_none() {
            let scope = self.scope.as_ref().ok_or("Custody scope missing")?;
            let client = self.client.as_ref().ok_or("Custody execution missing")?;
            if self.worker_start.is_none() {
                let mut permit = Some(session.control().take_worker_permit()?);
                let request = spec(2, BACKEND_CUSTODY_POOL_BYTES)?;
                self.worker_start =
                    Some(client.reserve_backend_custody_worker(scope, request, &mut permit)?);
                self.terminals.worker_original =
                    Some(super::terminal::Original::new(scope, request));
            }
            if !self.capture.admitted() {
                if self.capture_input.is_none() {
                    self.capture_input = session.control().take_captured_input();
                }
                self.capture.start(
                    client,
                    self.capture_scope.as_ref().ok_or("Helper capture scope missing")?,
                    &mut self.capture_input,
                )?;
                if self.capture_input.is_some() {
                    return Err("Actual helper capture input retained without admission".into());
                }
            }
            if !self.capture.ready() {
                return Ok(());
            }
            let start =
                self.worker_start.as_ref().ok_or("Original custody worker start missing")?;
            let source = BackendCustodySource::new(
                start.metadata(),
                self.role.installation.take().ok_or("Custody installation missing")?,
            )?;
            self.source = Some(source.clone());
            let driver = source.driver(&mut self.session)?;
            let start = self.worker_start.take().ok_or("Original custody worker start missing")?;
            // The fixed one-driver factory stays in the original funded metadata envelope.
            let factory: DriverFactory = Box::new(move |_, _| Ok(driver));
            match start.bind(factory) {
                Ok(job) => self.worker = Some(job),
                Err((start, factory)) => {
                    self.worker_start = Some(start);
                    self.rejected_factory = Some(factory);
                    return Err("Custody driver binding rejected with original custody".into());
                }
            }
        }
        Ok(())
    }
    /// Keep unresolved native custody observable; only its authenticated self owner can exit.
    fn observe(&mut self) -> Result<HeadlessStep, Box<dyn Error>> {
        if let Some(source) = &self.source
            && let Some(cause) = source.failure()
        {
            if source.unresolved() && !source.recovery_capture_pending() {
                source.exit_unresolved_helper()?;
            }
            return Err(Box::new(cause));
        }
        if let Some(worker) = &mut self.worker
            && let Some(completion) = worker.try_take_completion()
        {
            self.terminals.worker(completion, self.closing, self.role.key)?;
            return Ok(HeadlessStep::Shutdown);
        }
        Ok(HeadlessStep::Active)
    }
    /// Preserve pending capture measurement and apply its first cutoff before propagating errors.
    fn advance_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        let observed = if let Some(snapshot) = self.capture.cleanup_snapshot() {
            let result = self.cleanup.observe(snapshot);
            self.until = self.cleanup.until();
            result
        } else {
            if !self.uncaptured_sampled && self.execution.is_some() {
                self.uncaptured_sampled = true;
                self.until = Some(
                    self.client
                        .as_ref()
                        .ok_or("Custody execution absent")?
                        .now()?
                        .checked_add(std::time::Duration::from_secs(15))?,
                );
            }
            Ok(())
        };
        if let Some(source) = &self.source {
            self.until = source.retirement_until().or_else(|| source.recovery_retirement_until());
        }
        if self.source.as_ref().is_none_or(|source| !source.recovery_capture_pending())
            && let (Some(execution), Some(until)) = (&mut self.execution, self.until)
        {
            execution.request_shutdown(until);
        }
        observed
    }
}
impl Lifetime {
    /// Observe every original owner before returning the first external delivery failure.
    fn poll_retirement(
        &mut self,
        drained: Option<Result<CompletionBatch, ExecutionError>>,
    ) -> Result<bool, Box<dyn Error>> {
        let mut failure = None;
        if let Some(drained) = drained {
            match drained {
                Ok(batch) => retain(&mut failure, self.capture.observe(&batch)),
                Err(cause) => retain(&mut failure, Err(Box::new(cause))),
            }
        }
        retain(&mut failure, self.advance_shutdown());
        if let Some(session) = &mut self.session {
            for fact in session.drain(NonZeroUsize::MAX) {
                drop(fact);
            }
            if let Some(done) = session.take_terminal() {
                retain(
                    &mut failure,
                    self.terminals.pipe(done, self.closing, self.authenticated, self.role.key),
                );
                self.session = None;
            }
        }
        if let Some(worker) = &mut self.worker
            && let Some(done) = worker.try_take_completion()
        {
            retain(&mut failure, self.terminals.worker(done, self.closing, self.role.key));
            self.worker = None;
        }
        if let Some(source) = &self.source
            && source.unresolved()
        {
            let pending_capture = source.recovery_capture_pending();
            let mut exit = source.failure().is_some() && !pending_capture;
            if !exit && let (Some(until), Some(client)) = (self.until, &self.client) {
                match client.now() {
                    Ok(now) => exit = now >= until,
                    Err(cause) => {
                        retain(&mut failure, Err(Box::new(cause)));
                        exit = true;
                    }
                }
            }
            if exit {
                retain(
                    &mut failure,
                    source
                        .exit_unresolved_helper()
                        .map(|never| match never {})
                        .map_err(|cause| Box::new(cause) as Box<dyn Error>),
                );
            }
        }
        let capture = match self.capture.settled() {
            Ok(settled) => settled,
            Err(cause) => {
                retain(&mut failure, Err(cause));
                false
            }
        };
        let execution = if self.until.is_some() {
            match self.execution.as_ref().map(|owner| owner.poll_shutdown()).transpose() {
                Ok(progress) => progress.is_none_or(|progress| progress.settled),
                Err(cause) => {
                    retain(&mut failure, Err(Box::new(cause)));
                    false
                }
            }
        } else {
            self.execution.is_none()
        };
        failure.map_or(Ok(capture && execution), Err)
    }
}
impl HeadlessLifetime for Lifetime {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>> {
        if self.execution.is_some() || self.closing {
            return Err("Custody lifetime cannot start twice".into());
        }
        self.execution = Some(ExecutionOwner::new(MonotonicClock::new(), wake)?);
        let client = self.execution.as_ref().ok_or("Custody execution absent")?.client();
        self.client = Some(client.clone());
        let scope =
            client.create_native_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        self.scope = Some(scope);
        self.capture_scope =
            Some(client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?);
        let request = spec(1, MIN_LIFECYCLE_PIPE_BYTES)?;
        let permit = client.reserve_captured_backend_custody_pipe_client(
            self.scope.as_ref().ok_or("Custody scope absent")?,
            request,
            self.role.original,
        )?;
        self.terminals.pipe_original = Some(super::terminal::Original::new(
            self.scope.as_ref().ok_or("Custody scope absent")?,
            request,
        ));
        self.session = permit.bind(self.role.input.take().ok_or("Custody inherited input absent")?);
        Ok(())
    }
    fn prepare_native_runtime(
        &mut self,
        max: NonZeroUsize,
    ) -> Result<StartupPreparationStep, Box<dyn Error>> {
        self.drain(max)?;
        self.authenticate()?;
        Ok(if self.worker.is_some() {
            StartupPreparationStep::Ready
        } else {
            StartupPreparationStep::Waiting
        })
    }
    fn activity(&mut self, max: NonZeroUsize) -> Result<HeadlessStep, Box<dyn Error>> {
        self.drain(max)?;
        self.observe()
    }
    fn next_deadline(&self) -> Option<Instant> {
        None
    }
    fn request_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        if self.closing {
            return self.advance_shutdown();
        }
        self.closing = true;
        self.capture.request_cleanup();
        drop(self.capture_input.take());
        drop(self.rejected_factory.take());
        if let Some(start) = self.worker_start.take() {
            self.worker = start.cancel();
        }
        if let Some(session) = &self.session {
            session.control().close();
        }
        if let Some(worker) = &self.worker {
            worker.cancel();
        }
        self.advance_shutdown()
    }
    fn poll_shutdown(&mut self) -> Result<bool, Box<dyn Error>> {
        let drained =
            self.client.as_ref().map(|client| client.drain_completions(NonZeroUsize::MAX));
        self.poll_retirement(drained)
    }
    fn finish_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(owner) = &mut self.execution {
            owner.finish_shutdown()?;
        }
        Ok(())
    }
}
/// Preserve the first cause while independent original owners remain observable.
fn retain(failure: &mut Option<Box<dyn Error>>, result: Result<(), Box<dyn Error>>) {
    if let Err(cause) = result {
        if failure.is_none() {
            *failure = Some(cause);
        } else {
            tracing::error!(operation="backend_custody_retirement",cause=%cause,"Additional owned helper retirement failure");
        }
    }
}
/// Retain the fixed native purpose/class without a persistent job deadline.
fn spec(id: u64, bytes: u64) -> Result<JobSpec, Box<dyn Error>> {
    Ok(JobSpec {
        id: JobId::new(id)?,
        operation: Operation::NativeLifecycle,
        class: JobClass::NativeObservation,
        deadline: None,
        reservation_bytes: bytes,
        replacement: None,
    })
}

#[cfg(test)]
#[path = "lifetime_tests.rs"]
mod tests;
