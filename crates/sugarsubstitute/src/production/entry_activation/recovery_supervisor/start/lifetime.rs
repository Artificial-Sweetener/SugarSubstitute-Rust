// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Commit the exact held carrier once and retain control/watch custody through original cleanup.
use super::*;
use std::num::NonZeroUsize;
impl DesktopStart {
    /// Progress only actual independent native enrollment under the original thirty-second jobs.
    pub(in crate::production::entry_activation) fn progress_recovery(
        &mut self,
        resources: &mut EntryResources<'_>,
    ) -> Result<()> {
        let path = match &self.projection.outcome {
            JobOutcome::Success(value) => &value.capture_root,
            _ => return Err("Actual recovery projection missing".into()),
        };
        let Some(mode) = &mut self.recovery else {
            return Ok(());
        };
        let state = mode.state_mut()?;
        let Some(session) = &mut state.active else {
            return Ok(());
        };
        if state.retired.is_some() {
            return Ok(());
        }
        let progress = session.poll(path, resources.client.now()?);
        state.owner_failed |= progress.owner_failed;
        state.session_failed |= progress.failed;
        state.handler_lost |= progress.handler_lost;
        if state.closing.is_none() && !state.session_failed {
            let registry =
                resources.authority.registry().ok_or("Actual recovery registry missing")?;
            let result = if progress.handler_needed {
                match session.prepare_handler(registry) {
                    Ok(()) => session.submit_handler(
                        resources.client,
                        resources.finite,
                        worker_spec(resources, &state.grant)?,
                    ),
                    Err(cause) => Err(cause),
                }
            } else if progress.desktop_needed {
                match session.prepare_desktop(registry) {
                    Ok(()) => session.submit_desktop(
                        resources.client,
                        resources.finite,
                        worker_spec(resources, &state.grant)?,
                    ),
                    Err(cause) => Err(cause),
                }
            } else {
                Ok(())
            };
            match result {
                Ok(()) | Err(SessionCode::Pending) => {}
                Err(cause) => {
                    state.session_failed = true;
                    if state.cause.is_none() {
                        state.cause = Some(Box::new(cause));
                    }
                }
            }
        }
        if progress.retired {
            state.retired = Some(session.take_retired()?);
        }
        if state.session_failed && !state.failure_reported {
            let (first, secondary, omitted) = if let Some(retired) = &state.retired {
                let (secondary, omitted) = retired.secondary_failure();
                (retired.failure().is_some(), secondary.is_some(), omitted)
            } else {
                let (secondary, omitted) = session.secondary_failure();
                (session.failure().is_some(), secondary.is_some(), omitted)
            };
            tracing::error!(operation="recovery_session_failed",run=?self.run,
                first_present=first||state.cause.is_some(),secondary_present=secondary,omitted,
                "Original recovery session causes remain retained through checked cleanup");
            state.failure_reported = true;
        }
        if state.owner_failed && state.closing.is_none() {
            return Err("Actual recovery native owner failed; original cause retained".into());
        }
        Ok(())
    }
    /// Borrow actual Root-enrolled UI authority for one nonblocking lifecycle-owner call.
    pub(in crate::production::entry_activation) fn with_recovery_desktop<T>(
        &self,
        read: impl FnOnce(&RecoveryEnrolledDesktop) -> T,
    ) -> Result<Option<T>> {
        let Some(mode) = &self.recovery else {
            return Ok(None);
        };
        mode.with_desktop(read)
    }
    /// Report a failed startup/control owner for exact-record cleanup, separate from fatal metadata.
    pub(in crate::production::entry_activation) fn recovery_failed(&self) -> bool {
        self.recovery
            .as_ref()
            .and_then(|mode| mode.state.as_ref())
            .is_some_and(|state| state.session_failed)
    }
    /// Report actual independent handler loss while the existing UI/control remain live.
    pub(in crate::production::entry_activation) fn recovery_handler_lost(&self) -> bool {
        self.recovery
            .as_ref()
            .and_then(|mode| mode.state.as_ref())
            .is_some_and(|state| state.handler_lost)
    }
    /// Expose only actual fatal native owner metadata to the existing Root fatal-disposition path.
    pub(in crate::production::entry_activation) fn recovery_owner_failed(&self) -> bool {
        self.recovery
            .as_ref()
            .and_then(|mode| mode.state.as_ref())
            .is_some_and(|state| state.owner_failed)
    }
    /// Close actual control and watch lifetimes under the caller's unchanged original transaction.
    /// Profile/family containment remains with the existing outer owner.
    pub(in crate::production::entry_activation) fn close_recovery(
        &mut self,
        resources: &mut EntryResources<'_>,
        budget: &ShutdownBudget,
    ) -> Result<bool> {
        let Some(mode) = &mut self.recovery else {
            return Ok(true);
        };
        let state = mode.state_mut()?;
        Closing::latch(&mut state.closing, *budget)?;
        let close = state.closing.as_mut().ok_or("Original recovery cleanup budget missing")?;
        if close.counter.is_none() {
            close.counter =
                Some(resources.client.encode_counter_deadline(close.budget.settle_until)?);
        }
        if state.retired.is_some() {
            return Ok(true);
        }
        if let Some(session) = &mut state.active {
            let close = state.closing.as_ref().ok_or("Original recovery cleanup budget missing")?;
            session.begin_close(
                close.counter.ok_or("Original recovery cleanup counter missing")?,
                close.budget.settle_until,
            )?;
            self.progress_recovery(resources)?;
            return Ok(self.recovery_settled());
        }
        mode.close_precommit()
    }
}
impl RecoveryStart {
    /// Borrow actual active or checked-retired independent UI authority without retaining a lock.
    fn with_desktop<T>(
        &self,
        read: impl FnOnce(&RecoveryEnrolledDesktop) -> T,
    ) -> Result<Option<T>> {
        let state = self.state()?;
        let result = if let Some(retired) = &state.retired {
            retired.with_desktop(read)
        } else if let Some(active) = &state.active {
            active.with_desktop(read)
        } else {
            return Ok(None);
        };
        match result {
            Ok(value) => Ok(Some(value)),
            Err(SessionCode::Pending) => Ok(None),
            Err(cause) => Err(cause.into()),
        }
    }
    /// Route the exact UI proof to ParentLifecycle while allowing disjoint profile/resource borrows.
    pub(in crate::production::entry_activation) fn deliver_ui_exit(
        &mut self,
        parent: &mut ParentLifecycle,
        resources: &ParentResources<'_, '_>,
    ) -> Result<bool> {
        let state = self.state_mut()?;
        if state.exit_delivered {
            return Ok(false);
        }
        let read = |desktop: &RecoveryEnrolledDesktop| {
            if desktop.actual_exit().is_none() {
                return Ok(false);
            }
            parent.recovery_desktop_exit(desktop, resources)?;
            Ok::<_, crate::production::parent_lifecycle::ParentError>(true)
        };
        let result = if let Some(retired) = &state.retired {
            retired.with_desktop(read)
        } else if let Some(active) = &state.active {
            active.with_desktop(read)
        } else {
            return Ok(false);
        };
        let delivered = match result {
            Ok(result) => result?,
            Err(SessionCode::Pending) => false,
            Err(cause) => return Err(cause.into()),
        };
        state.exit_delivered |= delivered;
        Ok(delivered)
    }
    /// Release unstarted pins and observe real terminal/proof custody, without creating another budget.
    /// An active session must be closed through DesktopStart with the original ShutdownBudget.
    pub(in crate::production::entry_activation) fn close_precommit(&mut self) -> Result<bool> {
        let state = self.state_mut()?;
        if state.retired.is_some() {
            return Ok(true);
        }
        if state.active.is_some() {
            return Ok(false);
        }
        if !state.precommit_closed {
            state.grant.close_admission();
            state.input = None;
            state.paired = None;
            state.bootstrap = None;
            state.enrollment = None;
            state.precommit_closed = true;
        }
        if state.cancelled_ui.is_none()
            && let Some(watch) = &mut state.desktop_watch
        {
            state.cancelled_ui = Some(watch.cancel_observation()?);
        }
        if let Some(permit) = state.handler_permit.take() {
            state.cancelled_handler = Some(permit.cancel_unbound());
        }
        observe_cancelled(&mut state.cancelled_ui, &mut state.ui_terminal)?;
        observe_cancelled(&mut state.cancelled_handler, &mut state.handler_terminal)?;
        if let Some(pipe) = &mut state.pipe {
            pipe.control().close();
            for observation in pipe.drain(NonZeroUsize::new(32).unwrap_or(NonZeroUsize::MIN)) {
                drop(observation);
            }
            if state.pipe_terminal.is_none() {
                state.pipe_terminal = pipe.take_terminal();
            }
            if state.pipe_retirement.is_none() {
                state.pipe_retirement = pipe.control().take_checked_retirement();
            }
            if state
                .pipe_terminal
                .as_ref()
                .is_some_and(|done| done.scope_id != pipe.scope_id() || done.spec != pipe.spec())
            {
                return Err("Actual recovery control terminal identity mismatch".into());
            }
            if state.pipe_retirement.as_ref().is_some_and(|proof| {
                !proof.matches(pipe.control())
                    || proof.key() != state.grant.key()
                    || proof.scope_id() != pipe.scope_id()
                    || proof.spec() != pipe.spec()
                    || proof.execution_id() != state.execution
            }) {
                return Err("Actual recovery checked retirement identity mismatch".into());
            }
        }
        for done in [&state.ui_terminal, &state.handler_terminal].into_iter().flatten() {
            state.owner_failed |= matches!(&done.outcome, JobOutcome::Panicked(_))
                || matches!(&done.outcome,JobOutcome::Failure(cause) if cause.contains_owner_panic());
            state.session_failed |= matches!(
                &done.outcome,
                JobOutcome::Failure(_) | JobOutcome::Panicked(_) | JobOutcome::Coalesced { .. }
            );
        }
        if let Some(done) = &state.pipe_terminal {
            state.owner_failed |= matches!(&done.outcome, JobOutcome::Panicked(_))
                || matches!(&done.outcome,JobOutcome::Failure(cause) if cause.contains_owner_panic()||cause.requires_process_shutdown());
            state.session_failed |= matches!(
                &done.outcome,
                JobOutcome::Failure(_) | JobOutcome::Panicked(_) | JobOutcome::Coalesced { .. }
            );
        }
        if state.owner_failed && state.closing.is_none() {
            return Err(
                "Actual recovery cleanup native owner failed; original terminal retained".into()
            );
        }
        Ok(state.settled())
    }
}
fn observe_cancelled(
    job: &mut Option<OwnedJob<ProcessExit, NativeObservationError>>,
    terminal: &mut Option<JobCompletion<ProcessExit, NativeObservationError>>,
) -> Result<()> {
    let Some(job) = job else {
        return Ok(());
    };
    if terminal.is_none() {
        *terminal = job.try_take_completion();
    }
    if terminal
        .as_ref()
        .is_some_and(|done| done.scope_id != job.scope_id() || done.spec != job.spec())
    {
        return Err("Actual recovery unused watch terminal identity mismatch".into());
    }
    Ok(())
}
fn worker_spec(resources: &EntryResources<'_>, grant: &RegisteredRecoveryGrant) -> Result<JobSpec> {
    Ok(JobSpec {
        id: resources.jobs.allocate()?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(grant.original_budget().until()),
        reservation_bytes: RootRecoverySession::worker_reservation_bytes(),
        replacement: None,
    })
}
