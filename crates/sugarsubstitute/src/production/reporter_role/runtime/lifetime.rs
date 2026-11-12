// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the sole real execution and native UI lifetime through authenticated report retirement.
use super::*;
impl ReportSessionContent for ReporterSession {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), ReportSurfaceError> {
        if self.started {
            return Err(Self::rejected("reporter_restarted"));
        }
        self.started = true;
        self.execution = Some(
            ExecutionOwner::new(MonotonicClock::new(), wake)
                .map_err(|cause| ReportSurfaceError::new("reporter_execution", cause))?,
        );
        let client = self
            .execution
            .as_ref()
            .ok_or_else(|| Self::rejected("reporter_execution_missing"))?
            .client();
        self.client = Some(client.clone());
        self.native = Some(
            client
                .create_native_scope(
                    ScopeSpec {
                        id: ScopeId::new(1).map_err(|cause| {
                            ReportSurfaceError::new("reporter_native_scope", cause)
                        })?,
                        deadline: None,
                    },
                    None,
                )
                .map_err(|cause| ReportSurfaceError::new("reporter_native_scope", cause))?,
        );
        self.finite = Some(
            client
                .create_scope(
                    ScopeSpec {
                        id: ScopeId::new(2).map_err(|cause| {
                            ReportSurfaceError::new("reporter_finite_scope", cause)
                        })?,
                        deadline: None,
                    },
                    None,
                )
                .map_err(|cause| ReportSurfaceError::new("reporter_finite_scope", cause))?,
        );
        let spec = JobSpec {
            id: self
                .jobs
                .allocate()
                .map_err(|cause| ReportSurfaceError::new("reporter_pipe_job", cause))?,
            operation: Operation::NativeLifecycle,
            class: JobClass::NativeObservation,
            deadline: None,
            reservation_bytes: sugarsubstitute_execution::purpose_pipe::MIN_LIFECYCLE_PIPE_BYTES,
            replacement: None,
        };
        self.channel
            .reserve(
                &client,
                self.native
                    .as_ref()
                    .ok_or_else(|| Self::rejected("reporter_native_scope_missing"))?,
                spec,
            )
            .map_err(|cause| ReportSurfaceError::new("reporter_pipe_admission", cause))?;
        let until = self
            .channel
            .original_until()
            .ok_or_else(|| Self::rejected("reporter_original_missing"))?;
        self.original = Some(until);
        let now = client
            .now()
            .map_err(|cause| ReportSurfaceError::new("reporter_preparation_clock", cause))?;
        self.deadline =
            Instant::now().checked_add(until.checked_duration_since(now).map_err(|cause| {
                ReportSurfaceError::new("reporter_preparation_deadline", cause)
            })?);
        let spec = JobSpec {
            id: self
                .jobs
                .allocate()
                .map_err(|cause| ReportSurfaceError::new("reporter_constructor_job", cause))?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(until),
            reservation_bytes: STORAGE_BYTES,
            replacement: None,
        };
        self.constructor = Some(
            client
                .submit_blocking(
                    self.finite.as_ref().ok_or_else(|| Self::rejected("reporter_scope_missing"))?,
                    spec,
                    |context| {
                        let metadata = context.memory().claim(METADATA_BYTES)?;
                        Ok(Funding {
                            view: ViewService::new(&context)?,
                            ui: context.memory().clone(),
                            _metadata: metadata,
                        })
                    },
                )
                .map_err(|cause| {
                    ReportSurfaceError::new("reporter_constructor_admission", cause)
                })?,
        );
        Ok(())
    }
    fn poll(&mut self, maximum: NonZeroUsize) -> Result<ReportSurfaceUpdate, ReportSurfaceError> {
        self.consume(maximum)?;
        Ok(self.update.take().unwrap_or(ReportSurfaceUpdate::None))
    }
    fn action(
        &mut self,
        request: ReportRequestId,
        command: ReportCommand,
    ) -> Result<(), ReportSurfaceError> {
        if self.closing || self.pending.is_some() || self.current != Some(command.binding()) {
            return Err(Self::rejected("reporter_action_binding"));
        }
        let wire = self.enqueue(command.binding(), Intent::Action(command.action()))?;
        self.pending = Some((request, command, wire));
        Ok(())
    }
    fn observe(&mut self, observation: ReportSurfaceObservation) -> Result<(), ReportSurfaceError> {
        match observation {
            ReportSurfaceObservation::Presented(binding)
            | ReportSurfaceObservation::ViewReplaced(binding) => {
                if self.current != Some(binding) {
                    return Err(Self::rejected("reporter_presentation_binding"));
                }
                self.funding
                    .as_ref()
                    .ok_or_else(|| Self::rejected("reporter_funding_missing"))?
                    .view
                    .presented(binding)
                    .map_err(|_| Self::rejected("reporter_presentation_pending"))?;
                self.enqueue(
                    binding,
                    if matches!(observation, ReportSurfaceObservation::Presented(_)) {
                        Intent::Presented
                    } else {
                        Intent::Replaced
                    },
                )?;
            }
            ReportSurfaceObservation::CopyFinished { binding, outcome, .. } => {
                self.enqueue(
                    binding,
                    Intent::Copy(if outcome.is_ok() {
                        DiagnosticActionResult::Success
                    } else {
                        DiagnosticActionResult::Failed(ReportFailure::Presentation)
                    }),
                )?;
            }
            ReportSurfaceObservation::SaveSelected(selection) => {
                let binding = selection.binding();
                let request = selection.request();
                if self.pending.is_some() || self.current != Some(binding) {
                    return Err(Self::rejected("reporter_save_binding"));
                }
                let command =
                    ReportCommand::new(binding, ReportAction::Export(selection.content()));
                let wire = self.enqueue(binding, Intent::Save(selection))?;
                self.pending = Some((request, command, wire));
            }
            ReportSurfaceObservation::SaveCancelled { binding, .. } => {
                self.enqueue(binding, Intent::Cancelled)?;
            }
            ReportSurfaceObservation::Failed { binding, failure } => {
                self.enqueue(binding, Intent::Failed(failure))?;
            }
            ReportSurfaceObservation::NativeRetired(session) => {
                let binding =
                    self.current.ok_or_else(|| Self::rejected("reporter_retirement_view"))?;
                if !self.closing || session != binding.session() || self.native_retired {
                    return Err(Self::rejected("reporter_retirement_identity"));
                }
                self.native_retired = true;
                self.enqueue(binding, Intent::Retired)?;
            }
        }
        Ok(())
    }
    fn request_shutdown(&mut self) -> Result<(), ReportSurfaceError> {
        if !self.closing {
            self.closing = true;
            if !self.awaiting_surface {
                self.native_retired = true;
            }
            if let Some(funding) = &self.funding {
                funding.view.cancel();
            }
            self.deadline = Instant::now().checked_add(Duration::from_secs(15));
            if let Some(client) = &self.client {
                self.shutdown_until = Some(
                    client
                        .now()
                        .map_err(|cause| ReportSurfaceError::new("reporter_shutdown_clock", cause))?
                        .checked_add(Duration::from_secs(15))
                        .map_err(|cause| {
                            ReportSurfaceError::new("reporter_shutdown_deadline", cause)
                        })?,
                );
            }
        }
        Ok(())
    }
    fn poll_shutdown(&mut self) -> Result<bool, ReportSurfaceError> {
        self.consume(NonZeroUsize::MIN.saturating_add(31))?;
        if self.native_retired && self.output_failed && self.encoding.is_none() {
            // These queued loans were never issued to the transport. Actual native UI
            // retirement precedes their disposal; issued writes still need their facts.
            self.outgoing.iter_mut().for_each(|input| {
                drop(input.take());
            });
        }
        if !self.native_retired
            || self.encoding.is_some()
            || self.written.is_some()
            || self.outgoing.iter().any(Option::is_some)
            || self.constructor.is_some()
            || self.funding.as_ref().is_some_and(|funding| !funding.view.worker_delivered())
        {
            return Ok(false);
        }
        self.received = None;
        self.view = None;
        self.update = None;
        self.encoded = None;
        self.funding = None;
        self.construction = None;
        self.channel.close();
        if !self.channel.terminal_delivered() {
            return Ok(false);
        }
        if let Some(execution) = &mut self.execution {
            if !self.execution_stopping {
                let until = self
                    .shutdown_until
                    .ok_or_else(|| Self::rejected("reporter_shutdown_budget_missing"))?;
                execution.request_shutdown(until);
                self.execution_stopping = true;
            }
            return execution
                .poll_shutdown()
                .map(|progress| progress.settled)
                .map_err(|cause| ReportSurfaceError::new("reporter_shutdown", cause));
        }
        Ok(true)
    }
    fn finish_shutdown(&mut self) -> Result<(), ReportSurfaceError> {
        if !self.poll_shutdown()? {
            return Err(Self::rejected("reporter_finish_unsettled"));
        }
        if let Some(execution) = &mut self.execution {
            execution
                .finish_shutdown()
                .map_err(|cause| ReportSurfaceError::new("reporter_join", cause))?;
        }
        Ok(())
    }
}
impl ReportSessionPreparation for ReporterSession {
    fn prepare(
        &mut self,
        maximum: NonZeroUsize,
    ) -> Result<Option<ReportSurfaceOptions>, ReportSurfaceError> {
        self.consume(maximum)?;
        if let Some(view) = self.view.take() {
            let lease = self
                .funding
                .as_ref()
                .ok_or_else(|| Self::rejected("reporter_ui_funding"))?
                .ui
                .clone();
            let options =
                ReportSurfaceOptions::new(view, self.locale.as_str(), true, 1000, 720, lease)?;
            self.awaiting_surface = true;
            self.deadline = None;
            Ok(Some(options))
        } else {
            Ok(None)
        }
    }
    fn preparation_deadline(&self) -> Option<Instant> {
        self.deadline
    }
}
