// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose actual symbol reception before the first same-session Reporter launch.
use super::*;
impl IncidentReporting {
    /// Prepare the original actual UTC/context once; all later work keeps this same first cutoff.
    pub(in crate::production::entry_activation::incident_reporting) fn prepare_original(
        &mut self,
        resources: &EntryResources<'_>,
    ) -> Result<bool> {
        if self.report_budget.is_none() {
            let budget = IncidentOperationBudget::new(resources.client.now()?)?;
            self.report_budget = Some(budget);
            self.queue_until = Some(budget.until());
        }
        let original = self.report_budget.ok_or("Original actual report interval missing")?;
        if self.receiving_session.preparing_mut()?.metadata.job.is_none()
            && self.receiving_session.preparing()?.metadata.completion.is_none()
        {
            let client = resources.client.clone();
            self.receiving_session.preparing_mut()?.metadata.job =
                Some(resources.client.submit_blocking(
                    resources.finite,
                    JobSpec {
                        id: resources.jobs.allocate()?,
                        operation: Operation::NativeLifecycle,
                        class: JobClass::InfrastructureLifecycle,
                        deadline: Some(original.until()),
                        reservation_bytes: 16 * 1024,
                        replacement: None,
                    },
                    move |context| delivery::prepare_material(&client, original, &context),
                )?);
        }
        let Some(material) = self.receiving_session.preparing()?.metadata.value()? else {
            return Ok(false);
        };
        self.original = Some(material.context);
        Ok(true)
    }
    /// Retain the genuine original effect before the readonly native pair; never renew its interval.
    pub(in crate::production::entry_activation::incident_reporting) fn advance_symbol_receiving(
        &mut self,
        host: &mut IncidentHost,
        resources: &EntryResources<'_>,
    ) -> Result<bool> {
        if self.symbols.is_none() {
            let needed = match host.next_effect()? {
                Some(IncidentEffect::LaunchReporter { summary, .. }) => {
                    matches!(summary.facts().capture(), CaptureEvidence::Finished(_))
                        && !matches!(summary.facts().symbols(), SymbolOutcome::Resolved(_))
                }
                _ => false,
            };
            if !needed && self.retained_effect.is_none() {
                return Ok(true);
            }
            if !self.prepare_original(resources)? {
                return Ok(false);
            }
            if self.retained_effect.is_none() {
                self.retained_effect = host.take_effect()?;
            }
            let binding = match self.retained_effect.as_ref() {
                Some(IncidentEffect::LaunchReporter { binding, .. }) => *binding,
                _ => return Err("Original queued Symbol launch changed".into()),
            };
            let original = self.original.ok_or("Actual original Symbol interval unavailable")?;
            let read = match host.admit_report_view(binding, original) {
                Ok(scope) => scope,
                Err(crate::production::incident_host::HostIssue::Pending) => return Ok(false),
                Err(cause) => return Err(cause.into()),
            };
            let mut request = Some(host.namespace()?.committed_read_request(read.operation())?);
            self.symbols = Some(SymbolReceiving::new(&self.retained_effect, read, &mut request)?);
        }
        let symbols = self.symbols.as_mut().ok_or("Original Symbol receiving owner missing")?;
        if !symbols.advance_native(resources, false)? {
            if symbols.result_delivered() && symbols.native_failed() {
                return Err(
                    "Actual Symbol retirement remains uncertain with original capsule retained"
                        .into(),
                );
            }
            return Ok(false);
        }
        if symbols.ready() {
            return Ok(true);
        }
        if symbols.native_failed() {
            return Err("Actual Symbol retirement failed with original capsule retained".into());
        }
        if self.store.is_none() {
            let original = self.original.ok_or("Original Symbol CAS context missing")?;
            let scope = match symbols.admit_metadata(host, original) {
                Ok(Some(scope)) => scope,
                Ok(None) => return Ok(false),
                Err(cause) => return Err(cause),
            };
            let binding = symbols.binding();
            let input = symbols.take_metadata_input(host)?;
            self.store = Some(StoreWork {
                scope,
                binding,
                kind: StoreKind::Symbols,
                input,
                installed: false,
                submitted: false,
                view_submitted: false,
                cleanup_until: None,
                cleanup_submitted: false,
                wire: None,
                counter: delivery::Delivery::new(),
                observer: None,
                adopted: false,
                prepared: None,
                stream_plan: None,
                stream_until: None,
            });
        }
        if let Some(ready) = self.advance_passive_symbols(host)? {
            if !ready {
                self.check_passive_cutoff(resources)?;
            }
            return Ok(ready);
        }
        if let Err(cause) = self.advance_store(host, resources) {
            if let Some(ready) = self.advance_passive_symbols(host)? {
                if !ready {
                    self.check_passive_cutoff(resources)?;
                }
                return Ok(ready);
            }
            return Err(cause);
        }
        if let Some(ready) = self.advance_passive_symbols(host)? {
            if !ready {
                self.check_passive_cutoff(resources)?;
            }
            return Ok(ready);
        }
        if self.store.as_ref().is_some_and(|work| matches!(work.kind, StoreKind::Symbols))
            && resources.client.now()?
                >= self.symbols.as_ref().ok_or("Original Symbol owner missing")?.until()
        {
            return Err("Original Symbol metadata cutoff elapsed with actual input retained".into());
        }
        Ok(self.symbols.as_ref().is_some_and(SymbolReceiving::ready))
    }
    /// Borrow the actual original or CAS-adopted effect; matching fields do not create one.
    pub(in crate::production::entry_activation::incident_reporting) fn next_launch_binding(
        &mut self,
        host: &mut IncidentHost,
    ) -> Result<Option<ReportBinding>> {
        let effect = match self.symbols.as_ref() {
            Some(symbols) => symbols.ready().then_some(()).and(self.retained_effect.as_ref()),
            None => host.next_effect()?,
        };
        Ok(match effect {
            Some(IncidentEffect::LaunchReporter { binding, .. }) => Some(*binding),
            _ => None,
        })
    }
    /// Move only the retained real launch, preserving current incident and same original session.
    pub(in crate::production::entry_activation::incident_reporting) fn take_next_launch(
        &mut self,
        host: &mut IncidentHost,
    ) -> Result<Option<IncidentEffect>> {
        match &self.symbols {
            Some(symbols) if symbols.ready() => Ok(self.retained_effect.take()),
            Some(_) => Ok(None),
            None => Ok(host.take_effect()?),
        }
    }
}
