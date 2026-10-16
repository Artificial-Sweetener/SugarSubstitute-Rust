// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit only finite actual family duplication/enrollment; retain effects before result transfer.
use super::*;
impl RootRecoverySession {
    /// Loan the actual started carrier family on Root before any handler worker admission.
    pub(in crate::production::entry_activation::recovery_supervisor) fn prepare_handler(
        &mut self,
        registry: &WindowsProcessRegistry,
    ) -> Result<(), SessionCode> {
        let state = self.state.as_mut().ok_or(SessionCode::Closed)?;
        if state.failures.first.is_some()
            || state.closing.is_some()
            || state.handler_advisory.is_none()
        {
            return Err(SessionCode::Order);
        }
        let shared = state.work.clone();
        let mut work = shared.try_lock().map_err(|_| SessionCode::Pending)?;
        if work.handler.is_some() {
            return Ok(());
        }
        let prepared = {
            let Work { desktop_watch, handler_permit, .. } = &mut *work;
            let anchor = desktop_watch.as_ref().ok_or(SessionCode::Order)?;
            let permit = handler_permit.take().ok_or(SessionCode::Order)?;
            registry.prepare_recovery_handler_watch(state.carrier, &state.grant, anchor, permit)
        };
        match prepared {
            Ok(handler) => {
                work.handler = Some(handler);
                Ok(())
            }
            Err(rejected) => {
                work.handler_permit = Some(rejected.permit);
                drop(work);
                state.record(SessionCause::Registry(rejected.cause));
                Err(SessionCode::Binding)
            }
        }
    }
    /// Prepare exact Root-local UI family ownership on the actor; never send a registry to a worker.
    pub(in crate::production::entry_activation::recovery_supervisor) fn prepare_desktop(
        &mut self,
        registry: &WindowsProcessRegistry,
    ) -> Result<(), SessionCode> {
        let state = self.state.as_mut().ok_or(SessionCode::Closed)?;
        if state.failures.first.is_some()
            || state.closing.is_some()
            || state.desktop_advisory.is_none()
        {
            return Err(SessionCode::Order);
        }
        let shared = state.work.clone();
        let mut work = shared.try_lock().map_err(|_| SessionCode::Pending)?;
        if work.desktop_prepared {
            return Ok(());
        }
        if work.handler.as_ref().is_none_or(|handler| {
            handler.actual().is_none()
                || handler.terminal().is_some()
                || handler.failure().is_some()
        }) {
            return Err(SessionCode::Order);
        }
        let result =
            work.desktop_watch.as_mut().ok_or(SessionCode::Order)?.prepare_binding(registry);
        match result {
            Ok(()) => {
                work.desktop_prepared = true;
                Ok(())
            }
            Err(cause) => {
                drop(work);
                state.record(SessionCause::Registry(cause));
                Err(SessionCode::Binding)
            }
        }
    }
    /// Admit the one original finite handler-binding attempt; rejection retains all native inputs.
    pub(in crate::production::entry_activation::recovery_supervisor) fn submit_handler(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
    ) -> Result<(), SessionCode> {
        self.submit_worker(client, scope, spec, WorkerKind::Handler)
    }
    /// Admit Root's actual still-suspended UI watch and direct lifecycle enrollment before reply.
    pub(in crate::production::entry_activation::recovery_supervisor) fn submit_desktop(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
    ) -> Result<(), SessionCode> {
        self.submit_worker(client, scope, spec, WorkerKind::Desktop)
    }
    fn submit_worker(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        kind: WorkerKind,
    ) -> Result<(), SessionCode> {
        let state = self.state.as_mut().ok_or(SessionCode::Closed)?;
        if state.failures.first.is_some()
            || state.closing.is_some()
            || state.job.is_some()
            || state.job_terminals[kind.index()].is_some()
            || spec.class != JobClass::InfrastructureLifecycle
            || spec.operation != Operation::NativeLifecycle
            || spec.deadline != Some(state.grant.original_budget().until())
            || spec.reservation_bytes < Self::worker_reservation_bytes()
            || spec.replacement.is_some()
        {
            return Err(SessionCode::Order);
        }
        let expected = match kind {
            WorkerKind::Handler => state.handler_advisory,
            WorkerKind::Desktop => state.desktop_advisory,
        }
        .ok_or(SessionCode::Order)?;
        {
            let work = state.work.try_lock().map_err(|_| SessionCode::Pending)?;
            if match kind {
                WorkerKind::Handler => work.handler.is_none(),
                WorkerKind::Desktop => !work.desktop_prepared,
            } {
                return Err(SessionCode::Order);
            }
        }
        let work = state.work.clone();
        match client.submit_blocking(scope, spec, move |context| {
            let mut work = work.lock().map_err(|_| SessionCode::Worker)?;
            bind(&mut work, kind, expected, &context)
        }) {
            Ok(job) => {
                state.job = Some(Worker { kind, job });
                Ok(())
            }
            Err(cause) => {
                state.record(SessionCause::Admission(cause));
                Err(SessionCode::Worker)
            }
        }
    }
}
fn bind(
    work: &mut Work,
    kind: WorkerKind,
    expected: RecoveryProcessIdentity,
    context: &JobContext,
) -> Result<(), SessionCode> {
    let result = match kind {
        WorkerKind::Handler => {
            work.handler.as_mut().ok_or(SessionCode::Order)?.bind(expected, context)
        }
        WorkerKind::Desktop => {
            let desktop = match work
                .desktop_watch
                .as_mut()
                .ok_or(SessionCode::Order)?
                .bind(expected, context)
            {
                Ok(desktop) => desktop,
                Err(cause) => {
                    work.failure[kind.index()] = Some(cause);
                    return Err(SessionCode::Worker);
                }
            };
            // Adopt the whole actual watch/family before another fallible native lifecycle transfer.
            work.desktop = Some(desktop);
            let Work { desktop, enrollment, .. } = work;
            desktop.as_mut().ok_or(SessionCode::Order)?.bind_lifecycle(enrollment, context)
        }
    };
    match result {
        Ok(()) => Ok(()),
        Err(cause) => {
            work.failure[kind.index()] = Some(cause);
            Err(SessionCode::Worker)
        }
    }
}

impl State {
    pub(super) fn poll_worker(&mut self) {
        let Some(worker) = &mut self.job else {
            return;
        };
        let Some(done) = worker.job.try_take_completion() else {
            return;
        };
        let index = worker.kind.index();
        let exact = done.scope_id == worker.job.scope_id() && done.spec == worker.job.spec();
        let panic = match &done.outcome {
            JobOutcome::Panicked(cause) => Some(cause.clone()),
            _ => None,
        };
        let success = matches!(&done.outcome, JobOutcome::Success(()));
        self.job_terminals[index] = Some(done);
        self.job = None;
        if let Some(cause) = panic {
            self.record(SessionCause::Panic(SessionPanic(cause)));
        }
        let shared = self.work.clone();
        let work = match shared.try_lock() {
            Ok(work) => work,
            Err(TryLockError::WouldBlock) => {
                self.record(SessionCause::Protocol(SessionCode::Worker));
                return;
            }
            Err(TryLockError::Poisoned(poison)) => poison.into_inner(),
        };
        let original = work.failure[index].as_ref().cloned();
        let actual = if index == 0 {
            work.handler.as_ref().and_then(RecoveryHandlerWatch::actual).is_some()
        } else {
            work.desktop.as_ref().is_some_and(RecoveryEnrolledDesktop::lifecycle_enrolled)
        };
        drop(work);
        if let Some(cause) = original {
            self.record(SessionCause::Registry(cause));
        }
        if !exact || !success || !actual {
            self.record(SessionCause::Protocol(SessionCode::Worker));
        }
    }
}
