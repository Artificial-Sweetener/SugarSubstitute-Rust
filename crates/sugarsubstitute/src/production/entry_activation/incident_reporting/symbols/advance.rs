// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive the actual original paired Symbol service without draining or waiting for jobs.
use super::*;
impl SymbolReceiving {
    /// Admit only the genuine bounded native pair, then require its actual ordered checked result.
    /// Closing cancels the same Main or held factory and still observes its actual cleanup delivery.
    pub(super) fn advance_native(
        &mut self,
        resources: &EntryResources<'_>,
        closing: bool,
        transactions: &mut DiagnosticTransactions,
    ) -> Result<bool> {
        self.closing |= closing;
        let runtime = (resources.finite.owner_id(), resources.finite.id());
        if self.runtime.is_some_and(|original| original != runtime) {
            self.first.get_or_insert(SymbolReceivingCause::State);
        } else {
            self.runtime = Some(runtime);
        }
        if self.delivered {
            let settled = self.native_settled();
            if settled {
                transactions.retired(self.read_scope.operation(), &mut self.transaction)?;
            }
            return Ok(settled);
        }
        if self.service.is_none() && (closing || self.first.is_some()) {
            transactions.retired(self.read_scope.operation(), &mut self.transaction)?;
            self.ready = !closing;
            return Ok(true);
        }
        if self.service.is_none() {
            if resources.client.now()? >= self.until() {
                return Err("Original Symbol namespace admission cutoff elapsed".into());
            }
            if !transactions.admit(
                DiagnosticTransactionRole::Symbols,
                self.read_scope.operation(),
                &mut self.transaction,
            )? {
                return Ok(false);
            }
            let cleanup = match resources.jobs.allocate() {
                Ok(id) => id,
                Err(cause) => {
                    self.first = Some(SymbolReceivingCause::Identity(cause));
                    transactions.retired(self.read_scope.operation(), &mut self.transaction)?;
                    self.ready = true;
                    return Ok(true);
                }
            };
            let main = match resources.jobs.allocate() {
                Ok(id) => id,
                Err(cause) => {
                    self.first = Some(SymbolReceivingCause::Identity(cause));
                    transactions.retired(self.read_scope.operation(), &mut self.transaction)?;
                    self.ready = true;
                    return Ok(true);
                }
            };
            match SymbolReportService::reserve(
                resources.client.clone(),
                resources.cleanup,
                cleanup,
                resources.finite,
                main,
                self.until(),
            ) {
                Ok(service) => self.service = Some(service),
                Err(cause) => {
                    self.record_admission(cause);
                    transactions.retired(self.read_scope.operation(), &mut self.transaction)?;
                    return Ok(self.service.is_none() && self.first.is_some());
                }
            }
        }
        if closing || self.first.is_some() {
            self.cancel_original();
        } else if !self.submitted {
            let service = self.service.as_mut().ok_or("Actual Symbol service missing")?;
            if self.request.is_some()
                && let Err(cause) = service.bind_request(&mut self.request)
            {
                self.first.get_or_insert(SymbolReceivingCause::Admission(cause));
            }
            if self.first.is_none() {
                match service.submit(resources.finite) {
                    Ok(()) => self.submitted = true,
                    Err(cause) => self.record_admission(cause),
                }
            }
            if self.first.is_some() {
                self.cancel_original();
            }
        }
        let service = self.service.as_mut().ok_or("Original Symbol service missing")?;
        if let Err(cause) = service.advance_cleanup() {
            self.cleanup_admission.get_or_insert(cause);
        }
        let Some(result) = service.take_result() else { return Ok(false) };
        self.delivered = true;
        match result {
            SymbolReportResult::Verified(observation) => self.observation = Some(observation),
            SymbolReportResult::Failed(failure) => {
                self.ready = !closing && failure.settled();
                self.failure = Some(failure);
            }
        }
        let settled = self.native_settled();
        if settled {
            transactions.retired(self.read_scope.operation(), &mut self.transaction)?;
        }
        Ok(settled)
    }
    fn record_admission(&mut self, cause: AdmissionError) {
        if matches!(
            cause,
            AdmissionError::QueueSaturated { .. }
                | AdmissionError::MemorySaturated { .. }
                | AdmissionError::CompletionSaturated
        ) {
            self.admission.get_or_insert(cause);
        } else {
            self.first.get_or_insert(SymbolReceivingCause::Admission(cause));
            if self.service.is_none() {
                self.ready = true;
            }
        }
    }
    /// Cancel only an actual Main or the original actual held factory, never fabricate its terminal.
    pub(super) fn cancel_original(&mut self) {
        let Some(service) = &mut self.service else { return };
        if self.submitted {
            service.cancel();
        } else if !self.cancelled {
            match service.cancel_unsubmitted() {
                Ok(()) => self.cancelled = true,
                Err(cause) => {
                    self.first.get_or_insert(SymbolReceivingCause::Admission(cause));
                }
            }
        }
    }
}
