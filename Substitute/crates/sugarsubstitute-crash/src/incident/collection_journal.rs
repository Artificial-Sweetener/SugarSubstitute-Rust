// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep exact committed files pinned until native capture settlement and journal absence.
use super::{
    CapturedReport, IncidentCommit, IncidentRepository, IncidentStoreError, IncidentStoreState,
};
use crate::native::finished_report::{JournalSource, SettlementFailure};
use sugarsubstitute_contracts::{execution::JobContext, incident::*};
use sugarsubstitute_supervision::diagnostic_storage::*;
pub(crate) struct JournalState {
    record: IncidentStoreState,
    payload_revision: IncidentRevision,
    summary_index: usize,
    payload_index: usize,
    journal_index: Option<usize>,
    stage_index: Option<usize>,
    activation_started: bool,
    write_uncertain: bool,
    activation: Option<DiagnosticActivation>,
    intent: Option<DiagnosticCaptureIntent>,
    settled: bool,
    journal_retired: bool,
    payload_closed: bool,
    summary_closed: bool,
    failure: Option<DiagnosticCaptureRetirementError<SettlementFailure>>,
}
impl CapturedReport {
    /// Borrow the original native disposition or checked journal-retirement failure.
    pub fn journal_failure(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.journal.as_ref().and_then(|journal| journal.failure.as_ref()).map(|cause| cause as _)
    }
    /// Project only the journal's original moved native/control leaf, without traversing errors.
    pub fn journal_failure_diagnostic(&self) -> Option<super::FinishedFailureDiagnostic> {
        self.journal.as_ref()?.failure.as_ref()?.source_error().map(SettlementFailure::diagnostic)
    }
    /// Project actual journal storage rejection independently of its original native cause.
    pub fn journal_storage_failure(&self) -> Option<DiagnosticStorageFailureCode> {
        self.journal
            .as_ref()?
            .failure
            .as_ref()?
            .post_storage_error()
            .map(DiagnosticStorageError::code)
    }
    /// Observe an opaque journal post-provider unwind without formatting its payload.
    pub fn journal_post_panicked(&self) -> bool {
        self.journal
            .as_ref()
            .and_then(|journal| journal.failure.as_ref())
            .is_some_and(|failure| failure.post_panicked())
    }
    /// Inspect the original opaque native-control unwind retained by settlement.
    pub fn inspect_settlement_panic<T>(
        &self,
        inspect: impl FnOnce(&(dyn std::any::Any + Send)) -> T,
    ) -> Option<T> {
        use std::error::Error;
        self.journal
            .as_ref()?
            .failure
            .as_ref()?
            .source()?
            .downcast_ref::<SettlementFailure>()?
            .inspect_panic(inspect)
    }
    /// Observe checked source/stage settlement and protected Journal absence.
    /// This grants no acknowledgement, native process settlement or restart permission.
    pub fn disposition_settled(&self) -> bool {
        self.journal.as_ref().is_some_and(|journal| journal.settled)
    }
}
impl IncidentRepository {
    /// Bind the actual immutable schema2 Summary and payload to prepared native metadata.
    /// The exact canonical Journal is activated and strictly read before adopting its fence.
    /// # Errors
    /// Every read/stage/activation uncertainty retains its original owner; retries cannot
    /// replace the Summary, payload generation or historical operation.
    pub fn publish_capture_journal(
        &mut self,
        report: &mut CapturedReport,
        committed: &IncidentCommit,
        context: &JobContext,
    ) -> Result<(), IncidentStoreError> {
        let token = report.token.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        self.publish_capture_journal_token(token, &mut report.journal, committed, context)
    }
    pub(crate) fn publish_capture_journal_token(
        &mut self,
        token: &mut crate::native::finished_report::Token,
        retained: &mut Option<JournalState>,
        committed: &IncidentCommit,
        context: &JobContext,
    ) -> Result<(), IncidentStoreError> {
        if committed.operation() != self.operation() {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        if retained.is_none() {
            *retained = Some(self.prepare_capture_journal(token, committed, context)?);
        }
        let journal = retained.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        let IncidentStoreState::CapturePromotion(plan) = &journal.record else {
            return Err(IncidentStoreError::RecoveryConflict);
        };
        if plan.incident() != committed.key()
            || plan.incident_revision() != committed.revision()
            || journal.payload_revision
                != committed.payload_revision().ok_or(IncidentStoreError::MissingPayload)?
            || plan.historical_operation() != self.operation()
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        if journal.intent.is_some() || journal.settled {
            return Ok(());
        }
        if journal.activation.is_none() {
            let mut bytes = self.storage.record_buffer(&self.operation, context)?;
            let mut decode = self.storage.recovery_decode(&self.operation, context)?;
            super::allocation::decode(&mut decode, || {
                super::codec_ledger::write_store_state(&journal.record, &mut bytes)
            })?;
            drop(decode);
            if journal.stage_index.is_none() {
                self.inventory(context)?;
                let index = self.stages.len();
                self.stages.push(self.storage.stage(
                    DiagnosticObject::Journal(self.operation()),
                    bytes.as_bytes().len() as u64,
                    &self.operation,
                    context,
                )?)?;
                journal.stage_index = Some(index);
                // The entire already-admitted journal fits one checked write. Failure
                // never writes its bytes again into an ambiguously advanced stream.
                journal.write_uncertain = true;
                self.stages.get_mut(index)?.write_chunk(bytes.as_bytes(), context)?;
                journal.write_uncertain = false;
            } else if journal.write_uncertain {
                return Err(IncidentStoreError::RecoveryConflict);
            }
            let index = journal.stage_index.ok_or(IncidentStoreError::RecoveryConflict)?;
            let flushed = self.stages.get_mut(index)?.flush(context)?.read_facts();
            if flushed.bytes() != bytes.as_bytes().len() as u64
                || flushed.sha256() != super::codec_ledger::digest(bytes.as_bytes())
            {
                return Err(IncidentStoreError::WrongDigest);
            }
            let actual = if journal.activation_started {
                self.storage.reconcile_activation(
                    DiagnosticObject::Journal(self.operation()),
                    super::repository::predecessor(flushed),
                    &self.operation,
                    context,
                )?
            } else {
                journal.activation_started = true;
                self.storage.activate(
                    self.stages.get_mut(index)?,
                    DiagnosticPredecessor::Absent,
                    &self.operation,
                    context,
                )?
            };
            self.stages.remove(index)?;
            journal.stage_index = None;
            journal.activation = Some(actual);
        }
        if journal.journal_index.is_none() {
            let index = self.reads.len();
            self.reads.push(self.storage.read(
                DiagnosticObject::Journal(self.operation()),
                65_536,
                &self.operation,
                context,
            )?)?;
            journal.journal_index = Some(index);
        }
        let index = journal.journal_index.ok_or(IncidentStoreError::RecoveryConflict)?;
        let actual = self.reads.get_mut(index)?.facts();
        if Some(actual) != journal.activation.as_ref().map(DiagnosticActivation::facts) {
            return Err(IncidentStoreError::WrongDigest);
        }
        let bytes = self.reads.get_mut(index)?.whole_source(context)?;
        let mut decode = self.storage.recovery_decode(&self.operation, context)?;
        super::allocation::decode(&mut decode, || {
            if super::decode_store_state(bytes.as_bytes())? != journal.record {
                return Err(IncidentStoreError::RecoveryConflict);
            }
            Ok(())
        })?;
        drop(decode);
        drop(bytes);
        let (summary, payload, intent_file) =
            self.reads.three_mut(journal.summary_index, journal.payload_index, index)?;
        journal.intent = Some(token.adopt_capture_intent(
            &mut self.storage,
            intent_file,
            summary,
            Some(payload),
            &self.operation,
            context,
        )?);
        Ok(())
    }
    fn prepare_capture_journal(
        &mut self,
        token: &mut crate::native::finished_report::Token,
        committed: &IncidentCommit,
        context: &JobContext,
    ) -> Result<JournalState, IncidentStoreError> {
        if !self.reads.is_empty() || self.pending.is_some() {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        let revision = committed.payload_revision().ok_or(IncidentStoreError::MissingPayload)?;
        let (dump, bytes) = token.dump().ok_or(IncidentStoreError::MissingPayload)?;
        if committed.summary().facts().capture() != CaptureEvidence::Finished(dump) {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        let summary_index = self.reads.len();
        self.reads.push(self.storage.read(
            DiagnosticObject::Summary(committed.key()),
            65_536,
            &self.operation,
            context,
        )?)?;
        let summary_file = self.reads.get_mut(summary_index)?.facts();
        if summary_file != committed.actual.facts() {
            return Err(IncidentStoreError::StaleRevision);
        }
        let source = self.reads.get_mut(summary_index)?.whole_source(context)?;
        let mut decode = self.storage.recovery_decode(&self.operation, context)?;
        super::allocation::decode(&mut decode, || {
            let stored = super::decode_summary(source.as_bytes())?;
            if stored != committed.stored.stored || stored.payload_revision() != Some(revision) {
                return Err(IncidentStoreError::RecoveryConflict);
            }
            Ok(())
        })?;
        drop(decode);
        drop(source);
        let payload_index = self.reads.len();
        self.reads.push(self.storage.read(
            DiagnosticObject::Payload(committed.key(), revision, AttachmentKind::Minidump),
            bytes,
            &self.operation,
            context,
        )?)?;
        let payload_file = self.reads.get_mut(payload_index)?.facts();
        let plan = token.plan(committed.summary(), summary_file, payload_file)?;
        Ok(JournalState {
            record: IncidentStoreState::CapturePromotion(Box::new(plan)),
            payload_revision: revision,
            summary_index,
            payload_index,
            journal_index: None,
            stage_index: None,
            activation_started: false,
            activation: None,
            intent: None,
            settled: false,
            failure: None,
            write_uncertain: false,
            journal_retired: false,
            payload_closed: false,
            summary_closed: false,
        })
    }
    /// Invoke the same actual pinned native source, then checked-retire its protected Journal.
    /// # Errors
    /// Actual source/post-control/delete/close/absence failure stays with `report` and
    /// its original claim. Inspect `journal_failure`; matching facts never settle it.
    pub fn settle_capture_journal(
        &mut self,
        report: &mut CapturedReport,
        context: &JobContext,
    ) -> Result<(), IncidentStoreError> {
        let token = report.token.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        self.settle_capture_journal_token(token, &mut report.journal, context)
    }
    pub(crate) fn settle_capture_journal_token(
        &mut self,
        token: &mut crate::native::finished_report::Token,
        retained: &mut Option<JournalState>,
        context: &JobContext,
    ) -> Result<(), IncidentStoreError> {
        let journal = retained.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        if journal.settled {
            return Ok(());
        }
        let IncidentStoreState::CapturePromotion(plan) = &journal.record else {
            return Err(IncidentStoreError::RecoveryConflict);
        };
        if !journal.journal_retired {
            let mut source = JournalSource::new(token, plan, journal.payload_revision);
            match self.storage.settle_capture(
                journal.intent.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?,
                &mut source,
                &self.operation,
                context,
            ) {
                Ok(_) => {
                    let index =
                        journal.journal_index.take().ok_or(IncidentStoreError::RecoveryConflict)?;
                    self.reads.remove(index)?;
                    journal.journal_retired = true;
                }
                Err(cause) => {
                    if journal.failure.is_none() {
                        journal.failure = Some(cause);
                    }
                    return Err(IncidentStoreError::RecoveryConflict);
                }
            }
        }
        // Each checked closure is latched after the already-retired native Journal.
        // A later read-close failure never invokes CPP or deletes that Journal again.
        if !journal.payload_closed {
            self.close_read(journal.payload_index, context)?;
            journal.payload_closed = true;
        }
        if !journal.summary_closed {
            self.close_read(journal.summary_index, context)?;
            journal.summary_closed = true;
        }
        journal.settled = true;
        Ok(())
    }
}

mod recovery;
