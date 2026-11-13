// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt only genuinely unwritten Symbols work, preserving its complete native result and cause.
use super::*;
impl IncidentReporting {
    /// Keep the same original cutoff while actual generation adoption is waiting on custody.
    pub(super) fn check_passive_cutoff(&self, resources: &EntryResources<'_>) -> Result<()> {
        let symbols = self.symbols.as_ref().ok_or("Original passive Symbols owner missing")?;
        if resources.client.now()? >= symbols.until() {
            return Err(
                "Original passive Symbols cutoff elapsed with genuine custody retained".into()
            );
        }
        Ok(())
    }
    /// Reuse the same first interval after a sealed never-written disposition and actual pure adoption.
    /// Closing uses the separate close-only path and never allocates a replacement generation.
    pub(in crate::production::entry_activation::incident_reporting) fn advance_passive_symbols(
        &mut self,
        host: &mut IncidentHost,
        transactions: &mut DiagnosticTransactions,
    ) -> Result<Option<bool>> {
        let Some(symbols) = &mut self.symbols else { return Ok(None) };
        if symbols.closing || symbols.ready || symbols.passive.is_some() {
            return Ok(symbols.ready.then_some(true));
        }
        let Some(work) = &mut self.store else { return Ok(None) };
        if !matches!(work.kind, StoreKind::Symbols) || !work.installed && work.input.is_some() {
            return Ok(None);
        }
        let service = host.reporting_service()?;
        if service.passive_restoration().is_none() {
            let restored = match service.outcome() {
                None if !work.submitted
                    && (service.admission_failure().is_some()
                        || service.identity_failure().is_some()) =>
                {
                    service.restore_unsubmitted(&mut work.input)
                }
                Some(TerminalKind::Cancelled | TerminalKind::TimedOut) => {
                    service.restore_unstarted(&mut work.input)
                }
                _ => return Ok(None),
            };
            match restored {
                Ok(_) => {
                    symbols.restore_metadata(service, &mut work.input)?;
                    work.installed = false;
                }
                Err(ReportingIssue::Busy | ReportingIssue::Started) => return Ok(None),
                Err(cause) => return Err(cause.into()),
            }
        }
        let marker: PassiveReportingRestoration =
            service.passive_restoration().ok_or("Original passive Symbols marker missing")?;
        if !marker.is_symbols()
            || marker.operation() != work.scope.operation()
            || marker.binding() != work.binding
        {
            return Err("Original passive Symbols marker changed kind or scope".into());
        }
        transactions.retired(work.scope.operation(), &mut work.transaction)?;
        if symbols.adopted_passive.is_none() {
            if !service.validate_passive_restoration(marker) {
                return Ok(Some(false));
            }
            symbols.adopted_passive =
                Some(host.adopt_report_symbols_not_written(work.scope, work.binding, marker)?);
        }
        let service = host.reporting_service()?;
        let retained = match service.renew_passive_adopted(&mut symbols.adopted_passive) {
            Ok(retained) => retained,
            Err(ReportingIssue::Busy) => return Ok(Some(false)),
            Err(cause) => return Err(cause.into()),
        };
        if retained.marker() != marker || retained.outcome() != marker.terminal() {
            symbols.passive = Some(retained);
            return Err("Original no-write disposition changed during actual adoption".into());
        }
        symbols.passive = Some(retained);
        let (old_binding, old_summary) = match self.retained_effect.as_ref() {
            Some(IncidentEffect::LaunchReporter { binding, summary }) => (*binding, summary),
            _ => return Err("Original unwritten Symbols launch missing".into()),
        };
        match host.next_effect()? {
            Some(IncidentEffect::LaunchReporter { binding, summary })
                if *binding == old_binding && summary == old_summary => {}
            _ => return Err("Actual no-write Symbols successor changed original report".into()),
        }
        self.retained_effect = Some(host.take_effect()?.ok_or("Actual no-write launch consumed")?);
        symbols.ready = true;
        self.store = None;
        Ok(Some(true))
    }
}
