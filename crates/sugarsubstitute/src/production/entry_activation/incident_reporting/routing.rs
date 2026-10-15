// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route only the finite Core decoder's closed facts with their genuine original request tokens.
use super::*;
impl IncidentReporting {
    pub(super) fn capture_output(
        &mut self,
        host: &mut IncidentHost,
        resources: &EntryResources<'_>,
    ) -> Result<()> {
        match self.relay()?.source().poll() {
            Ok(facts) => {
                if facts.failed {
                    tracing::error!(
                        operation = "reporter_source_stopped",
                        original_cause_retained =
                            self.relay()?.source().failure(|cause| cause.is_some())?,
                        "Original Reporter native/source custody retained"
                    );
                }
                self.observe_source_facts(facts)?;
            }
            Err(ReporterSourceIssue::Busy) => return Ok(()),
            Err(cause) => return Err(cause.into()),
        }
        if !self.relay()?.worker_delivered() {
            return Ok(());
        }
        let output = self.relay.as_mut().ok_or("Actual report relay missing")?.take_output()?;
        if self.relay()?.worker_terminal().is_some() {
            let relay = self.relay()?;
            tracing::error!(
                operation = "reporter_core_stopped",
                panic_retained = relay.panic_cause().is_some(),
                admission_cause_retained = relay.admission_failure().is_some(),
                worker_cause_retained = relay.worker_failure().is_some(),
                "Original Reporter Core terminal and source custody retained"
            );
            return Err("Actual report Core relay failed with original completion retained".into());
        }
        if let Some(output) = output {
            self.relay_until = None;
            self.route(host, output)?;
        } else if self.relay_until.is_some() {
            self.relay_until = None;
        }
        let _resources = resources;
        Ok(())
    }
    fn route(&mut self, host: &mut IncidentHost, output: ReporterOutput) -> Result<()> {
        match output {
            ReporterOutput::Completed { received, issued, accepted } => {
                if !accepted {
                    self.rejected_completion = Some((received, issued));
                    return Err("Original Reporter router completion rejected".into());
                }
                if let Err(cause) =
                    self.accept_export_completion(host, received.key(), issued.request())
                {
                    self.rejected_completion = Some((received, issued));
                    return Err(cause);
                }
                drop(received);
                let _issued = issued;
                if self.store.as_ref().is_some_and(|work| work.adopted) {
                    host.reporting_service()?.release_adopted()?;
                    self.store = None;
                }
                self.relay()?.source().resume_read()?;
                self.relay_needed = true;
            }
            ReporterOutput::ControlWritten(_operation) => {
                self.relay_needed = false;
            }
            ReporterOutput::Received {
                received,
                admission,
                observation,
                request_context,
                observer,
            } => {
                if received.key() != self.key.ok_or("Actual Reporter key missing")? {
                    self.rejected_received = Some((received, admission));
                    return Err("Original Reporter session changed".into());
                }
                if self.request.is_some() || self.completion.is_some() || self.export_work.is_some()
                {
                    self.rejected_received = Some((received, admission));
                    return Err("Original Reporter request already retained".into());
                }
                let issued = match admission {
                    DiagnosticAdmission::Execute(issued) => issued,
                    DiagnosticAdmission::Pending | DiagnosticAdmission::Replay(_) => {
                        drop(received);
                        self.relay()?.source().resume_read()?;
                        self.relay_needed = true;
                        return Ok(());
                    }
                };
                self.request = Some(NativeRequest {
                    authentication: ReportingAuthentication { received, issued },
                    reply: observer,
                    context: request_context.request,
                });
                match observation {
                    ReporterObservation::Presented { binding, replacement: false } => {
                        if self.binding != Some(binding) {
                            return Err("Actual Presented binding changed".into());
                        }
                        self.original = Some(request_context.request);
                        host.observe_presented(binding, request_context.request)?;
                    }
                    ReporterObservation::Presented { binding, replacement: true } => {
                        if self.binding != Some(binding) {
                            return Err("Actual ViewReplaced binding changed".into());
                        }
                        host.observe_view_replaced(binding)?;
                        self.complete_observation()?;
                    }
                    ReporterObservation::Failed { binding, failure } => {
                        if self.binding != Some(binding) {
                            return Err("Actual ReporterFailed binding changed".into());
                        }
                        self.original = Some(request_context.request);
                        host.observe_report_failed(binding, failure, request_context.request)?;
                    }
                    ReporterObservation::Action(command) => {
                        if self.binding != Some(command.binding()) {
                            return Err("Actual report action binding changed".into());
                        }
                        let Some(copied) = restart_guard::admit(command, self.restart, || {
                            Ok(host.report_action(command, request_context.request)?)
                        })?
                        else {
                            self.reject_restart(command.binding(), request_context.relay_until)?;
                            self.complete_observation()?;
                            return Ok(());
                        };
                        self.original = Some(request_context.request);
                        self.dismiss_action = Some(command.action());
                        self.close_request = self
                            .request
                            .as_ref()
                            .map(|request| request.authentication.issued.request());
                        if let Some(binding) = copied {
                            if binding != command.binding() {
                                return Err("Original copy authorization binding changed".into());
                            }
                            self.authorize_copy(binding, request_context.relay_until)?;
                            self.complete_observation()?;
                        } else if matches!(command.action(), ReportAction::Export(_)) {
                            self.complete_observation()?;
                        }
                    }
                    ReporterObservation::Selected { binding, selection } => {
                        if self.binding != Some(binding) {
                            return Err("Actual selected view binding changed".into());
                        }
                        self.original = Some(request_context.request);
                        host.report_action(
                            ReportCommand::new(binding, ReportAction::Export(selection)),
                            request_context.request,
                        )?;
                        let request =
                            self.request.take().ok_or("Original selected frame missing")?;
                        self.selected = Some(SelectedReportExport {
                            binding,
                            context: request.context,
                            selection,
                            authentication: request.authentication,
                        });
                    }
                    ReporterObservation::NativeRetired(binding) => {
                        if self.binding != Some(binding) {
                            return Err("Actual NativeRetired binding changed".into());
                        }
                        self.native_retired = true;
                        self.complete_observation()?;
                    }
                    ReporterObservation::Copy { binding, result } => {
                        if self.binding != Some(binding) {
                            return Err("Actual native copy completion binding changed".into());
                        }
                        tracing::info!(
                            operation = "reporter_native_copy_result",
                            session = binding.session().get(),
                            succeeded = matches!(result, DiagnosticActionResult::Success),
                            "Actual native clipboard result observed"
                        );
                        self.complete_observation()?;
                    }
                    ReporterObservation::SelectionCancelled(binding) => {
                        if self.binding != Some(binding) {
                            return Err("Actual report action completion binding changed".into());
                        }
                        self.complete_observation()?;
                    }
                }
            }
        }
        Ok(())
    }
    fn complete_observation(&mut self) -> Result<()> {
        let original = self.request.take().ok_or("Original observer request missing")?;
        self.completion = Some(ReporterCompletion {
            received: original.authentication.received,
            issued: original.authentication.issued,
            reply: original.reply,
        });
        Ok(())
    }
    pub(super) fn drive_relay(&mut self, resources: &EntryResources<'_>) -> Result<()> {
        let source = self.relay()?.source().alias();
        if self.completion.is_some() {
            match source.install_completion(&mut self.completion) {
                Ok(()) => self.relay_needed = true,
                Err(ReporterSourceIssue::Busy) => {}
                Err(cause) => return Err(cause.into()),
            }
        }
        if self.pending_control.is_some() {
            match source.install_control(&mut self.pending_control) {
                Ok(()) => self.relay_needed = true,
                Err(ReporterSourceIssue::Busy) => {}
                Err(cause) => return Err(cause.into()),
            }
        }
        if !self.relay_needed || !self.relay()?.worker_delivered() {
            return Ok(());
        }
        let until = match self.relay_until {
            Some(until) => until,
            None => {
                let until = resources.client.now()?.checked_add(Duration::from_secs(60))?;
                self.relay_until = Some(until);
                until
            }
        };
        let relay = self.relay.as_mut().ok_or("Actual report relay missing")?;
        match relay.submit(resources.client, resources.finite, resources.jobs, until) {
            Ok(()) => self.relay_needed = false,
            Err(ReporterSourceIssue::Admission | ReporterSourceIssue::Busy) => {}
            Err(cause) => return Err(cause.into()),
        }
        Ok(())
    }
}
