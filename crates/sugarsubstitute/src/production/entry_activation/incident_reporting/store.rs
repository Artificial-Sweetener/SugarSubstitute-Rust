// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Serialize actual protected reads, guarded CAS and checked cleanup before pure adoption.
use super::*;
impl IncidentReporting {
    /// Retain one canonical locale projected from this registered failed desktop configuration.
    /// # Errors
    /// Rejects a changed locale while this report owner still retains the original selection.
    pub(in crate::production::entry_activation) fn configure_locale(
        &mut self,
        locale: sugarsubstitute_contracts::configuration::LocaleId,
    ) -> Result<()> {
        if self.locale.is_some_and(|original| original != locale) {
            return Err("Original registered report locale changed".into());
        }
        self.locale = Some(locale);
        Ok(())
    }
    pub(super) fn begin_store(
        &mut self,
        host: &mut IncidentHost,
        scope: IncidentOperationScope,
        binding: ReportBinding,
        kind: StoreKind,
        at: IncidentUtc,
    ) -> Result<()> {
        if self.store.is_some() {
            return Err("Report repository already occupied".into());
        }
        let work = match kind {
            StoreKind::View { .. } | StoreKind::ExportView => {
                let mut work = self.view_work(binding)?;
                if matches!(kind, StoreKind::ExportView)
                    && let ReportingWork::View { root_capture_degraded, .. } = &mut work
                {
                    *root_capture_degraded = false;
                }
                work
            }
            StoreKind::Attempt(attempt) => ReportingWork::Attempt { binding, attempt },
            StoreKind::Acknowledge(command) => ReportingWork::Acknowledge { command, at },
            StoreKind::Symbols => {
                return Err("Symbols requires the original sealed observation".into());
            }
        };
        let request = host.namespace()?.diagnostic_write_request(scope.operation())?;
        let original = self.request.take();
        let observer = original.as_ref().map(|request| request.reply);
        let authentication = original.map(|request| request.authentication);
        self.store = Some(StoreWork {
            transaction: None,
            scope,
            binding,
            kind,
            input: Some(ReportingInput {
                request,
                operation: scope.operation(),
                work,
                authentication,
            }),
            installed: false,
            submitted: false,
            view_submitted: false,
            cleanup_until: None,
            cleanup_submitted: false,
            wire: None,
            counter: delivery::Delivery::new(),
            observer,
            adopted: false,
            prepared: None,
            stream_plan: None,
            stream_until: None,
        });
        Ok(())
    }
    pub(super) fn consume_effect(
        &mut self,
        host: &mut IncidentHost,
        resources: &EntryResources<'_>,
    ) -> Result<()> {
        if self.store.is_some() || self.completion.is_some() || self.export_work.is_some() {
            return Ok(());
        }
        if !self.initial_view_admitted {
            if host.next_effect()?.is_some() {
                return Ok(());
            }
            let binding = self.binding.ok_or("Initial report binding missing")?;
            let context = self.original.ok_or("Original report preparation missing")?;
            let scope = host.admit_report_view(binding, context)?;
            self.begin_store(
                host,
                scope,
                binding,
                StoreKind::View { previous: None },
                context.at(),
            )?;
            self.initial_view_admitted = true;
            return Ok(());
        }
        if self.retained_effect.is_none() {
            self.retained_effect = host.take_effect()?;
        }
        let Some(effect) = self.retained_effect.as_ref() else { return Ok(()) };
        match effect {
            IncidentEffect::RecordAttempt { scope, binding, attempt, at } => {
                let (scope, binding, attempt, at) = (*scope, *binding, *attempt, *at);
                self.begin_store(host, scope, binding, StoreKind::Attempt(attempt), at)?;
                self.retained_effect = None;
            }
            IncidentEffect::Acknowledge { scope, binding, at } => {
                let (scope, binding, at) = (*scope, *binding, *at);
                let action =
                    self.request.as_ref().ok_or("Original acknowledgement request missing")?;
                let command = ReportCommand::new(
                    binding,
                    if action.context.at() == at {
                        self.dismiss_action.ok_or("Original dismissal intent missing")?
                    } else {
                        return Err("Original acknowledgement UTC changed".into());
                    },
                );
                self.begin_store(host, scope, binding, StoreKind::Acknowledge(command), at)?;
                self.retained_effect = None;
            }
            IncidentEffect::ReplaceReportView { old, current, .. } => {
                let (old, current) = (*old, *current);
                if self.binding != Some(old) {
                    return Err("Actual replacement predecessor mismatch".into());
                }
                let context =
                    self.original.ok_or("Original actual presentation interval missing")?;
                let scope = host.admit_report_view(current, context)?;
                self.begin_store(
                    host,
                    scope,
                    current,
                    StoreKind::View { previous: Some(old.revision()) },
                    context.at(),
                )?;
                self.retained_effect = None;
            }
            IncidentEffect::CloseAfterDurableAck { binding, acknowledgement } => {
                let (binding, acknowledgement) = (*binding, *acknowledgement);
                if self.binding != Some(binding) || self.pending_control.is_some() {
                    return Err("Actual durable close binding changed".into());
                }
                let original =
                    self.original.ok_or("Original durable acknowledgement interval missing")?;
                let counter =
                    self.close_counter.ok_or("Original acknowledgement close counter missing")?;
                let request =
                    self.close_request.ok_or("Original actual dismissal wire request missing")?;
                self.pending_control = Some(ReporterControlInput {
                    key: self.key.ok_or("Actual Reporter key missing")?,
                    request,
                    context: DiagnosticContext {
                        incident: Some(DiagnosticIncident {
                            key: acknowledgement.incident(),
                            revision: acknowledgement.revision(),
                        }),
                        deadline: Some(counter),
                        ..DiagnosticContext::default()
                    },
                    session: binding.session(),
                    until: original.budget().until(),
                    command: ReporterControl::Close(acknowledgement),
                });
                self.retained_effect = None;
            }
            IncidentEffect::Export { scope, binding, selection } => {
                let (scope, binding, selection) = (*scope, *binding, *selection);
                self.begin_export(host, scope, binding, selection, resources)?;
            }
            IncidentEffect::ForwardRestartIntent { .. }
            | IncidentEffect::ForwardRecoveredRestartIntent { .. } => {}
            _ => {}
        }
        let _resources = resources;
        Ok(())
    }
    pub(super) fn advance_store(
        &mut self,
        host: &mut IncidentHost,
        resources: &EntryResources<'_>,
        transactions: &mut DiagnosticTransactions,
    ) -> Result<()> {
        let Some(work) = &mut self.store else { return Ok(()) };
        let service = host.reporting_service()?;
        let until = work.scope.original().until();
        if !work.installed {
            if resources.client.now()? >= until {
                return Err("Original report namespace admission cutoff elapsed".into());
            }
            if !transactions.admit(
                DiagnosticTransactionRole::Reporting,
                work.scope.operation(),
                &mut work.transaction,
            )? {
                return Ok(());
            }
            let submitted = service.submit(
                resources.client,
                resources.finite,
                resources.jobs,
                until,
                &mut work.input,
            );
            work.installed = work.input.is_none();
            match submitted {
                Ok(()) => work.submitted = true,
                Err(ReportingIssue::Admission) => return Ok(()),
                Err(cause) => return Err(cause.into()),
            }
        }
        if !work.submitted {
            match service.retry(resources.client, resources.finite, resources.jobs, until) {
                Ok(()) => work.submitted = true,
                Err(ReportingIssue::Admission) => return Ok(()),
                Err(cause) => return Err(cause.into()),
            }
        }
        if service.outcome().is_none() {
            return Ok(());
        }
        if work.adopted {
            return Ok(());
        }
        if service.inspect(ReportingOperation::failed) == Some(true) {
            return Err(
                "Actual report repository operation failed with original cause retained".into()
            );
        }
        if !matches!(work.kind, StoreKind::ExportView | StoreKind::Symbols)
            && work.counter.job.is_none()
            && work.counter.completion.is_none()
        {
            let client = resources.client.clone();
            let cutoff = service.view_cutoff().unwrap_or(until);
            work.stream_until = Some(cutoff);
            work.counter.job = Some(resources.client.submit_blocking(
                resources.finite,
                JobSpec {
                    id: resources.jobs.allocate()?,
                    operation: Operation::NativeLifecycle,
                    class: JobClass::InfrastructureLifecycle,
                    deadline: Some(cutoff),
                    reservation_bytes: 16 * 1024,
                    replacement: None,
                },
                move |context| delivery::prepare_counter(&client, cutoff, &context),
            )?);
        }
        if !matches!(work.kind, StoreKind::ExportView | StoreKind::Symbols) {
            let Some(counter) = work.counter.value()? else { return Ok(()) };
            work.wire = Some(counter.counter);
        }
        if matches!(work.kind, StoreKind::View { .. } | StoreKind::ExportView) {
            if !work.view_submitted {
                match service.submit_view(
                    resources.client,
                    resources.finite,
                    resources.jobs,
                    service.view_cutoff().ok_or("Actual loaded view cutoff missing")?,
                ) {
                    Ok(()) => work.view_submitted = true,
                    Err(ReportingIssue::Admission) => return Ok(()),
                    Err(cause) => return Err(cause.into()),
                }
            }
            if service.view_outcome().is_none() {
                return Ok(());
            }
        }
        if work.cleanup_until.is_none() {
            work.cleanup_until =
                Some(resources.client.now()?.checked_add(Duration::from_secs(15))?);
        }
        if !work.cleanup_submitted {
            match service.submit_cleanup(
                resources.client,
                resources.cleanup,
                resources.jobs,
                work.cleanup_until.ok_or("Original report cleanup cutoff missing")?,
            ) {
                Ok(()) => work.cleanup_submitted = true,
                Err(ReportingIssue::Admission) => return Ok(()),
                Err(cause) => return Err(cause.into()),
            }
        }
        if !service.files_retired() {
            if service.cleanup_outcome().is_some() {
                return Err("Actual report checked file cleanup failed".into());
            }
            return Ok(());
        }
        transactions.retired(work.scope.operation(), &mut work.transaction)?;
        self.adopt_store(host)
    }
    fn adopt_store(&mut self, host: &mut IncidentHost) -> Result<()> {
        let work = self.store.as_ref().ok_or("Actual checked report operation missing")?;
        let (scope, binding) = (work.scope, work.binding);
        match work.kind {
            StoreKind::Symbols => self.adopt_symbols_store(host)?,
            StoreKind::ExportView => {
                let view = host
                    .reporting_service()?
                    .take_view()
                    .ok_or("Actual complete export view missing")?;
                self.install_export_view(view)?;
                host.reporting_service()?.release_adopted()?;
                self.store = None;
            }
            StoreKind::View { previous } => {
                if self.store.as_ref().is_some_and(|work| work.prepared.is_none()) {
                    let view = host
                        .reporting_service()?
                        .take_view()
                        .ok_or("Actual charged complete report view missing")?;
                    let request = self.allocate_wire()?;
                    let chunks = view.view().text().len().div_ceil(MAX_CHUNK_BYTES) as u64;
                    self.wire_request = self
                        .wire_request
                        .checked_add(chunks + 1)
                        .ok_or("Report stream request identity exhausted")?;
                    let work = self.store.as_mut().ok_or("Actual view store missing")?;
                    work.stream_plan = Some(ReporterViewPlan {
                        restart: self.restart,
                        key: self.key.ok_or("Actual report key missing")?,
                        first_request: request,
                        context: DiagnosticContext {
                            incident: Some(DiagnosticIncident {
                                key: binding.incident(),
                                revision: binding.revision(),
                            }),
                            deadline: work.wire,
                            ..DiagnosticContext::default()
                        },
                        until: work.stream_until.ok_or("Original view streaming cutoff missing")?,
                        previous,
                    });
                    work.prepared = Some(view);
                }
                let source = self.relay()?.source().alias();
                let work = self.store.as_mut().ok_or("Actual original stream missing")?;
                let plan = work.stream_plan.ok_or("Original stream context missing")?;
                match source.install_view(&mut work.prepared, plan) {
                    Ok(()) => {}
                    Err(ReporterSourceIssue::Busy) => return Ok(()),
                    Err(cause) => return Err(cause.into()),
                }
                host.reporting_service()?.release_adopted()?;
                self.binding = Some(binding);
                self.store = None;
                self.relay_needed = true;
            }
            StoreKind::Attempt(_) | StoreKind::Acknowledge(_) => {
                if !work.adopted {
                    match work.kind {
                        StoreKind::Attempt(_) => host.adopt_report_attempt(scope, binding)?,
                        StoreKind::Acknowledge(_) => host.adopt_acknowledgement(scope, binding)?,
                        _ => {}
                    };
                    self.store.as_mut().ok_or("Actual report operation missing")?.adopted = true;
                }
                let authentication = host
                    .reporting_service()?
                    .take_authentication()
                    .ok_or("Original report authentication still unavailable")?;
                let work = self.store.as_ref().ok_or("Actual adopted report missing")?;
                self.close_counter = work.wire;
                self.completion = Some(ReporterCompletion {
                    received: authentication.received,
                    issued: authentication.issued,
                    reply: work.observer.ok_or("Original observer receipt missing")?,
                });
            }
        }
        Ok(())
    }
}

impl IncidentReporting {
    /// Preserve the registered configuration locale for both displayed and exported report text.
    /// This inert work description grants no protected read or native copy privilege.
    pub(in crate::production::entry_activation) fn view_work(
        &self,
        binding: ReportBinding,
    ) -> Result<ReportingWork> {
        Ok(ReportingWork::View {
            binding,
            locale: self.locale.ok_or("Registered report locale missing")?.as_str(),
            root_capture_degraded: self.fresh_supervisor,
        })
    }
}
