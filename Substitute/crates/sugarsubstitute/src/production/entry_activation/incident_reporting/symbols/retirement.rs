// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Close the original Symbol pair and metadata owner even before a Reporter relay exists.
use super::*;
impl IncidentReporting {
    /// Advance actual Symbol retirement before any relay-only early return.
    /// Original inputs, failures, completed admissions and immutable first cutoffs stay retained.
    pub(in crate::production::entry_activation::incident_reporting) fn retire_symbols(
        &mut self,
        host: &mut IncidentHost,
        resources: &EntryResources<'_>,
        transactions: &mut DiagnosticTransactions,
    ) -> Result<bool> {
        let Some(symbols) = &mut self.symbols else { return Ok(true) };
        if !symbols.advance_native(resources, true, transactions)? {
            return Ok(false);
        }
        let Some(work) = &mut self.store else { return Ok(symbols.settled()) };
        if !matches!(work.kind, StoreKind::Symbols) {
            return Ok(true);
        }
        if !work.installed {
            // The identical unissued input remains owned here; no native writer existed.
            if work.input.is_some() {
                symbols.metadata_retired = true;
                transactions.retired(work.scope.operation(), &mut work.transaction)?;
            }
            return Ok(symbols.settled());
        }
        let service = if host.closing() {
            host.retiring_reporting_service()?
        } else {
            host.reporting_service()?
        };
        service.cancel();
        let restored = match service.outcome() {
            None => service.restore_unsubmitted(&mut work.input),
            Some(TerminalKind::Cancelled | TerminalKind::TimedOut) => {
                service.restore_unstarted(&mut work.input)
            }
            _ => Err(ReportingIssue::Started),
        };
        match restored {
            Ok(_) => {
                symbols.restore_metadata(service, &mut work.input)?;
                work.installed = false;
                transactions.retired(work.scope.operation(), &mut work.transaction)?;
                return Ok(symbols.settled());
            }
            Err(ReportingIssue::Busy | ReportingIssue::Started) => {}
            Err(cause) => return Err(cause.into()),
        }
        if service.outcome().is_none() {
            return Ok(false);
        }
        let budget = self.stopping.ok_or("Original Symbol close budget missing")?;
        let until = *work.cleanup_until.get_or_insert(budget.settle_until);
        if !service.cleanup_started() {
            match service.submit_cleanup(resources.client, resources.cleanup, resources.jobs, until)
            {
                Ok(()) => work.cleanup_submitted = true,
                Err(ReportingIssue::Admission | ReportingIssue::Busy) => return Ok(false),
                Err(cause) => return Err(cause.into()),
            }
        }
        if !service.files_retired() {
            return Ok(false);
        }
        if self.store_failure.is_none() {
            self.store_failure = service.take_failure();
        }
        transactions.retired(work.scope.operation(), &mut work.transaction)?;
        symbols.observe_metadata_retirement(service);
        Ok(symbols.settled())
    }
}
