// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit the actual Reporter role only from the queued committed collection effect.
use super::*;
impl IncidentReporting {
    pub(super) fn admit(
        &mut self,
        host: &mut IncidentHost,
        records: &mut [Option<RetainedRecord>; 64],
        resources: &EntryResources<'_>,
    ) -> Result<()> {
        if !self.advance_symbol_receiving(host, resources)? {
            return Ok(());
        }
        let Some(binding) = self.next_launch_binding(host)? else { return Ok(()) };
        let owner = resources.authority.key().ok_or("Actual report coordinator missing")?;
        if owner != binding.incident().subject().coordinator() {
            return Err("Actual report owner mismatch".into());
        }
        let IncidentSubject::Desktop(run) = binding.incident().subject() else {
            return Err("Live report source is not a desktop".into());
        };
        let record = records
            .iter_mut()
            .flatten()
            .find(|record| record.record.run == run)
            .ok_or("Genuine collected desktop source missing")?;
        let projected = record
            .record
            .start
            .as_mut()
            .ok_or("Actual report desktop projection missing")?
            .value_mut()?;
        self.configure_locale(projected.locale)?;
        if self.reporter_startup.is_none() {
            self.reporter_startup = Some(projected.registered.clone());
        }
        if self.relay.is_none() {
            let source = ReporterSource::new(host.reporter_funding()?)?;
            self.relay = Some(ReporterRelay::new(source));
        }
        self.binding = Some(binding);
        self.key = Some(DiagnosticKey::new(
            owner,
            DiagnosticRole::Reporter,
            DiagnosticSessionId::new(binding.session().get())?,
        ));
        match self.take_next_launch(host)? {
            Some(IncidentEffect::LaunchReporter { binding: actual, .. }) if actual == binding => {}
            _ => return Err("Queued Reporter effect changed".into()),
        }
        if self.report_budget.is_none() {
            let original = IncidentOperationBudget::new(resources.client.now()?)?;
            self.report_budget = Some(original);
            self.queue_until = Some(original.until());
        }
        Ok(())
    }
    pub(super) fn prepare_launch(
        &mut self,
        executable: &Path,
        working: &Path,
        resources: &mut EntryResources<'_>,
    ) -> Result<()> {
        if self.launched {
            return Ok(());
        }
        let until = self.queue_until.ok_or("Original report launch deadline missing")?;
        if !self.prepare_original(resources)? {
            return Ok(());
        }
        if self.session.is_none() && !self.session_installed {
            self.session = Some(resources.client.reserve_diagnostic_pipe_server(
                resources.native,
                JobSpec {
                    id: resources.jobs.allocate()?,
                    operation: Operation::NativeLifecycle,
                    class: JobClass::NativeObservation,
                    deadline: None,
                    reservation_bytes: MIN_LIFECYCLE_PIPE_BYTES,
                    replacement: None,
                },
                self.key.ok_or("Actual diagnostic key missing")?,
                until,
            )?);
        }
        if !self.session_installed {
            let source = self.relay()?.source().alias();
            source
                .install_owner(&mut self.session, self.key.ok_or("Actual report key missing")?)?;
            self.session_installed = true;
        }
        let source = self.relay()?.source().alias();
        if self.receiving_session.preparing_mut()?.launch.job.is_some() {
            return Ok(());
        }
        if self.receiving_session.preparing()?.launch.completion.is_none()
            && source.prepare(|_| ())?.is_none()
        {
            return Ok(());
        }
        if self.receiving_session.preparing_mut()?.launch.job.is_none()
            && self.receiving_session.preparing()?.launch.completion.is_none()
        {
            if self.launch_input.is_none() {
                self.launch_input = Some(delivery::LaunchDraft::new(
                    source,
                    executable.to_owned(),
                    working.to_owned(),
                    self.reporter_startup
                        .take()
                        .ok_or("Actual registered Reporter startup missing")?,
                    self.locale.ok_or("Registered report locale missing")?,
                ));
            }
            self.receiving_session.preparing_mut()?.launch.job =
                Some(resources.client.submit_blocking_owned_box(
                    resources.finite,
                    JobSpec {
                        id: resources.jobs.allocate()?,
                        operation: Operation::NativeLifecycle,
                        class: JobClass::InfrastructureLifecycle,
                        deadline: Some(until),
                        reservation_bytes: 512 * 1024,
                        replacement: None,
                    },
                    &mut self.launch_input,
                    delivery::prepare_launch,
                )?);
        }
        let Some(prepared) = self.receiving_session.preparing()?.launch.value()? else {
            return Ok(());
        };
        if !self.launched {
            let specs = ReporterLaunch::allocate(
                ScopeSpec { id: resources.scopes.allocate()?, deadline: None },
                resources.jobs,
                until,
                prepared.bytes,
            )?;
            self.relay()?.source().launch(
                resources.authority.registry_mut().ok_or("Actual report registry missing")?,
                resources.client,
                resources.finite,
                specs,
            )?;
            self.launched = true;
            self.process = self.relay()?.source().poll()?.process;
        }
        Ok(())
    }
}
