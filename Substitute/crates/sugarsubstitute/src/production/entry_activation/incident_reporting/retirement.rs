// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire actual repository, registered Reporter family and checked pipe independently of expiry.
use super::*;
impl IncidentReporting {
    /// Preserve the earliest explicit global close budget; never renew it for receiving work.
    pub(in crate::production::entry_activation) fn request_close(
        &mut self,
        budget: ShutdownBudget,
    ) {
        self.stopping = Some(self.stopping.map_or(budget, |old| ShutdownBudget {
            started: old.started.min(budget.started),
            graceful_until: old.graceful_until.min(budget.graceful_until),
            settle_until: old.settle_until.min(budget.settle_until),
        }));
        self.receiving_session.cancel();
        if let Some(relay) = &self.relay {
            relay.cancel();
        }
    }
    pub(super) fn retire(
        &mut self,
        host: &mut IncidentHost,
        resources: &mut EntryResources<'_>,
        transactions: &mut DiagnosticTransactions,
    ) -> Result<()> {
        if self.stopping.is_none() {
            self.request_close(ShutdownBudget::new(resources.client.now()?)?);
        }
        self.receiving_session.cancel();
        if !self.receiving_session.delivered() {
            return Ok(());
        }
        if !self.retire_symbols(host, resources, transactions)? {
            return Ok(());
        }
        if self.relay.is_none()
            || self.binding.is_none() && self.relay.as_ref().is_some_and(ReporterRelay::idle_funded)
        {
            return Ok(());
        }
        if self.family_retired {
            if !host.closing() {
                self.finish_accepted_session()?;
            }
            return Ok(());
        }
        if self.receiving_session.recycled().is_some() {
            return self.advance_recycled_retirement(host, resources);
        }
        if let Some(retired) = self.receiving_session.retired() {
            if !host.closing() {
                match host.observe_reporter_retired(&retired) {
                    Ok(()) => {}
                    Err(crate::production::incident_host::HostIssue::Pending) => return Ok(()),
                    Err(cause) => return Err(cause.into()),
                }
            }
            self.family_retired = true;
            self.store = None;
            return Ok(());
        }
        if self.stopping.is_none() {
            self.request_close(ShutdownBudget::new(resources.client.now()?)?);
        }
        let budget = self.stopping.ok_or("Original Reporter close budget missing")?;
        self.receiving_session.cancel();
        if !self.receiving_session.delivered() {
            return Ok(());
        }
        if !self.session_installed && self.session.is_none() {
            self.family_retired = true;
            return Ok(());
        }
        if !self.retire_export(host, resources, transactions)? {
            return Ok(());
        }
        if let Some(work) = &mut self.store {
            work.counter.cancel();
            if !work.counter.delivered() {
                return Ok(());
            }
            let service = if host.closing() {
                host.retiring_reporting_service()?
            } else {
                host.reporting_service()?
            };
            service.cancel();
            if work.installed {
                let restored = match service.outcome() {
                    None => service.restore_unsubmitted(&mut work.input),
                    Some(TerminalKind::Cancelled | TerminalKind::TimedOut) => {
                        service.restore_unstarted(&mut work.input)
                    }
                    _ => Err(ReportingIssue::Started),
                };
                match restored {
                    Ok(marker) => {
                        let expected = (
                            resources.finite.owner_id(),
                            resources.finite.id(),
                            work.scope.original().until(),
                        );
                        let input =
                            work.input.as_ref().ok_or("Restored original report input missing")?;
                        if marker.original() != expected
                            || marker.operation() != work.scope.operation()
                            || marker.binding() != work.binding
                            || input.operation != marker.operation()
                            || input.binding() != marker.binding()
                        {
                            return Err("Actual passive report restoration changed identity".into());
                        }
                        work.installed = false;
                    }
                    Err(ReportingIssue::Busy | ReportingIssue::Started) => {}
                    Err(cause) => return Err(cause.into()),
                }
            }
            if work.installed {
                if service.outcome().is_none() {
                    return Ok(());
                }
                let until = *work.cleanup_until.get_or_insert(budget.settle_until);
                if !service.cleanup_started() {
                    match service.submit_cleanup(
                        resources.client,
                        resources.cleanup,
                        resources.jobs,
                        until,
                    ) {
                        Ok(()) => {}
                        Err(ReportingIssue::Admission | ReportingIssue::Busy) => return Ok(()),
                        Err(cause) => return Err(cause.into()),
                    }
                }
                if !service.files_retired() {
                    return Ok(());
                }
                if self.store_failure.is_none() {
                    self.store_failure = service.take_failure();
                }
            }
            transactions.retired(work.scope.operation(), &mut work.transaction)?;
        }
        let relay = self.relay.as_ref().ok_or("Actual Reporter close owner missing")?;
        relay.cancel();
        if !relay.worker_delivered() {
            return Ok(());
        }
        match relay.source().close() {
            Ok(()) => {}
            Err(ReporterSourceIssue::Busy) => return Ok(()),
            Err(cause) => return Err(cause.into()),
        }
        let registry =
            resources.authority.registry_mut().ok_or("Actual Reporter registry missing")?;
        if let Some(process) = self.process {
            let actual = registry.snapshot(process)?;
            if !actual.settled {
                registry.begin_stop(
                    process,
                    ShutdownId::new(
                        self.key.ok_or("Actual Reporter key missing")?.session().get(),
                    )?,
                    budget,
                )?;
                if !actual.cleanup_pending && !actual.creation_pending {
                    let member = if actual.member_watch_required {
                        Some(JobSpec {
                            id: resources.jobs.allocate()?,
                            operation: Operation::NativeLifecycle,
                            class: JobClass::NativeObservation,
                            deadline: None,
                            reservation_bytes: 4096,
                            replacement: None,
                        })
                    } else {
                        None
                    };
                    let spec = JobSpec {
                        id: resources.jobs.allocate()?,
                        operation: Operation::NativeLifecycle,
                        class: JobClass::InfrastructureLifecycle,
                        deadline: Some(budget.settle_until),
                        reservation_bytes: 4096,
                        replacement: None,
                    };
                    match registry.submit_cleanup_step(
                        resources.client,
                        process,
                        resources.cleanup,
                        spec,
                        member,
                    ) {
                        Ok(_) => {}
                        Err(RegistryAdmissionError::Busy) => {}
                        Err(cause) => return Err(cause.into()),
                    }
                }
                return Ok(());
            }
        }
        // A completed incoming loan is returned only after its actual family and files close.
        if let Some(work) = &self.store
            && work.installed
        {
            let service = if host.closing() {
                host.retiring_reporting_service()?
            } else {
                host.reporting_service()?
            };
            drop(service.take_retiring_authentication());
        }
        if let Some(work) = &mut self.store
            && !work.installed
            && let Some(input) = &mut work.input
        {
            let service = if host.closing() {
                host.retiring_reporting_service()?
            } else {
                host.reporting_service()?
            };
            service.validate_retiring_input(
                work.scope.operation(),
                work.binding,
                work.submitted,
                input,
            )?;
            // The actual family is settled above; returning its frame permits checked pipe close.
            drop(input.authentication.take());
        }
        drop(self.request.take());
        drop(self.completion.take());
        drop(self.selected.take());
        self.return_retiring_export_inputs()?;
        let relay = self.relay.as_ref().ok_or("Actual Reporter close owner missing")?;
        drop(self.rejected_completion.take());
        drop(self.rejected_received.take());
        if !relay.source().return_closed_inputs(registry)? {
            return Ok(());
        }
        let facts = match relay.source().poll_retiring() {
            Ok(facts) => facts,
            Err(ReporterSourceIssue::Busy) => return Ok(()),
            Err(cause) => return Err(cause.into()),
        };
        if !facts.pipe_settled {
            return Ok(());
        }
        self.dispose_retired_export(host)?;
        if self.reporter_failure.is_none() {
            self.reporter_failure = self
                .relay
                .as_mut()
                .ok_or("Actual retired Reporter relay missing")?
                .take_retired_failure(registry)?;
        }
        let binding = self.binding.ok_or("Actual retired Reporter binding missing")?;
        if self.failure().is_none() && !host.closing() {
            let relay = self.relay.as_ref().ok_or("Actual Reporter recycling owner missing")?;
            if relay.recycling_ready(registry, self.key.ok_or("Actual Reporter key missing")?)? {
                self.receiving_session.retire_healthy_preparation(
                    binding,
                    self.key.ok_or("Actual Reporter key missing")?,
                )?;
                return self.advance_recycled_retirement(host, resources);
            }
        }
        if let Some(process) = self.process {
            let source =
                self.relay.as_ref().ok_or("Actual retired Reporter source missing")?.source();
            self.receiving_session.retire_original_family(
                registry,
                binding,
                self.key.ok_or("Actual Reporter key missing")?,
                source,
                process,
            )?;
            self.process = None;
        }
        if !host.closing()
            && let Some(retired) = self.receiving_session.retired()
        {
            match host.observe_reporter_retired(&retired) {
                Ok(()) => {}
                Err(crate::production::incident_host::HostIssue::Pending) => return Ok(()),
                Err(cause) => return Err(cause.into()),
            }
        }
        self.family_retired = true;
        self.store = None;
        Ok(())
    }
}
