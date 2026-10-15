// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Copy the same protected finished source before publishing its immutable summary.
use super::{IncidentRepository, IncidentStoreError, PublishTicket};
use crate::{capture::Shared, native::finished_report::Token};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use sugarsubstitute_contracts::{execution::JobContext, incident::*};
use sugarsubstitute_supervision::diagnostic_storage::*;

/// Retained finished-report worker state, including the original capture source.
/// Construction is restricted to the genuine registered Capture owner. Native failures
/// remain borrowed from this owner, independently of categorical repository rejection.
pub struct CapturedReport {
    pub(crate) source: Arc<Shared>,
    copy: Option<CopyState>,
    pub(super) journal: Option<super::collection_journal::JournalState>,
    pub(crate) generation: Option<ProtectedCaptureReadLease>,
    pub(crate) cpp: Option<ProtectedFinishedOwnerAllocation>,
    pub(crate) rust: Option<ProtectedFinishedRustRetention>,
    pub(crate) token: Option<Token>,
}
pub(crate) struct CopyState {
    ticket: u64,
    stage: usize,
    bytes: u64,
    offset: u64,
    sha256: Sha256,
    copied: Option<AttachmentFacts>,
    write_uncertain: bool,
    activation_started: bool,
}
impl CapturedReport {
    pub(crate) fn new(source: Arc<Shared>) -> Self {
        Self {
            source,
            copy: None,
            journal: None,
            generation: None,
            cpp: None,
            rust: None,
            token: None,
        }
    }
    /// Borrow the first genuine native/control cause retained by the source owner.
    pub fn failure(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.token.as_ref().and_then(Token::cause)
    }
    /// Project the original native/control leaf without formatting or moving its cause.
    /// This passive value grants no file, disposition, retirement or report acceptance.
    pub fn failure_diagnostic(&self) -> Option<super::FinishedFailureDiagnostic> {
        self.token.as_ref().and_then(Token::failure_diagnostic)
    }
    /// Borrow the original central QPC conversion cause.
    pub fn clock_failure(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.token.as_ref().and_then(Token::counter_failure).map(|value| value as _)
    }
    /// Borrow the original opaque provider unwind without converting it to text.
    pub fn panic_payload(&self) -> Option<&(dyn std::any::Any + Send)> {
        self.token.as_ref().and_then(Token::panic_payload)
    }
    /// Prepare the original and candidate native metadata stages before any journal.
    /// # Errors
    /// Partial preparation remains with the issued token and blocks a fabricated completion.
    pub fn prepare_disposition(&mut self, context: &JobContext) -> Result<(), IncidentStoreError> {
        self.token
            .as_mut()
            .ok_or(IncidentStoreError::RecoveryConflict)?
            .prepare(context)
            .map_err(|_| IncidentStoreError::RecoveryConflict)
    }
    /// Project the selected original dump, metadata and both actual prepared copies.
    /// This bounded data adds canonical Summary/Journal staging room; it grants no deletion.
    pub fn publication_headroom(&self) -> Option<u64> {
        self.token.as_ref().and_then(Token::publication_headroom)
    }
    /// Observe the exact parser-confirmed dump identity and whole source size.
    /// These facts grant no file, disposition or presentation authority.
    pub fn dump(&self) -> Option<(DumpId, u64)> {
        self.token.as_ref().and_then(Token::dump)
    }
    /// Query the exact subject/build from the retained registered native generation.
    /// # Errors
    /// Provider/native refusal retains its genuine cause and issued owner. A missing
    /// or busy result is an availability observation and grants no discard authority.
    pub fn open(
        &mut self,
        key: IncidentKey,
        build: &BuildIdentity,
        dump: Option<DumpId>,
        context: &JobContext,
    ) -> Result<CaptureEvidence, IncidentStoreError> {
        let token = self.token.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        match token
            .open(key, build, dump, context)
            .map_err(|_| IncidentStoreError::RecoveryConflict)?
        {
            0 => Ok(CaptureEvidence::Finished(
                token.dump().ok_or(IncidentStoreError::MissingPayload)?.0,
            )),
            1 => Ok(CaptureEvidence::Unavailable(CaptureAbsence::Missing)),
            2 => Ok(CaptureEvidence::Unavailable(CaptureAbsence::Busy)),
            3 => Ok(CaptureEvidence::Unavailable(CaptureAbsence::Corrupt)),
            4 => Ok(CaptureEvidence::Unavailable(CaptureAbsence::WrongIdentity)),
            5 => Ok(CaptureEvidence::Unavailable(CaptureAbsence::Oversized)),
            6 => Ok(CaptureEvidence::Unavailable(CaptureAbsence::DeadlineElapsed)),
            _ => Err(IncidentStoreError::RecoveryConflict),
        }
    }
    /// Checked-close the native token followed by the actual generation read lease.
    /// # Errors
    /// Quarantine or uncertain closure preserves the identical native/source owner.
    pub fn retire(
        &mut self,
        client: &sugarsubstitute_execution::ExecutionClient,
        context: &JobContext,
    ) -> Result<(), IncidentStoreError> {
        let Some(token) = self.token.as_mut() else {
            // These guards have never reached Token::open or any CPP invocation.
            drop(self.cpp.take());
            if let Some(generation) = &mut self.generation {
                generation.retire(context)?;
                self.generation = None;
            }
            drop(self.rust.take());
            return Ok(());
        };
        if !token.retire_cpp(client, context).map_err(|_| IncidentStoreError::RecoveryConflict)? {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        token.retire_source(context).map_err(|_| IncidentStoreError::RecoveryConflict)
    }
}
impl IncidentRepository {
    /// Prepare the genuine Capture source under this repository's sole original operation.
    /// # Errors
    /// Foreign/retired Capture, source preparation or fixed-role denial preserves output custody.
    pub fn prepare_finished(
        &mut self,
        capture: &crate::capture::FinishedCaptureSource,
        output: &mut Option<CapturedReport>,
        context: &JobContext,
    ) -> Result<(), IncidentStoreError> {
        capture.prepare_finished(output, &mut self.storage, &self.operation, context)
    }
    /// Stream one complete Minidump from the genuine retained finished Capture owner.
    /// The same pending ticket, original context and stage survive read/write failures.
    /// # Errors
    /// Changed context, incomplete reads, digest disagreement or native uncertainty
    /// preserve the original stage and report. Inspect `report.failure()` for native causes.
    pub fn append_finished(
        &mut self,
        ticket: &mut PublishTicket,
        report: &mut CapturedReport,
        context: &JobContext,
    ) -> Result<AttachmentFacts, IncidentStoreError> {
        let token = report.token.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        self.append_finished_token(ticket, token, &mut report.copy, context)
    }
    pub(crate) fn append_finished_token(
        &mut self,
        ticket: &mut PublishTicket,
        token: &mut Token,
        retained: &mut Option<CopyState>,
        context: &JobContext,
    ) -> Result<AttachmentFacts, IncidentStoreError> {
        let pending = self.pending(ticket)?;
        let (_, bytes) = token.dump().ok_or(IncidentStoreError::MissingPayload)?;
        let original = pending.context.clone();
        let object = DiagnosticObject::Payload(
            pending.seed.key(),
            pending.revision,
            AttachmentKind::Minidump,
        );
        if let Some(copy) = &*retained
            && let Some(facts) = copy.copied
        {
            if copy.ticket == ticket.id
                && copy.bytes == bytes
                && super::publication::same_context(&original, context)
                && token.matches(pending.seed.key(), pending.seed.build(), self.operation.key())
                && pending.payloads.iter().any(|(kind, retained, source, omitted, actual)| {
                    *kind == AttachmentKind::Minidump
                        && *retained == facts.retained_bytes()
                        && *source == facts.original_bytes()
                        && *omitted == 0
                        && actual.object() == object
                        && actual.facts().sha256() == *facts.sha256()
                })
            {
                return Ok(facts);
            }
            return Err(IncidentStoreError::RecoveryConflict);
        }
        if !super::publication::same_context(&original, context)
            || !token.matches(pending.seed.key(), pending.seed.build(), self.operation.key())
            || pending.payloads.len() >= MAX_ATTACHMENTS
            || (retained.as_ref().is_none_or(|copy| copy.copied.is_none())
                && pending.payloads.iter().any(|(kind, ..)| *kind == AttachmentKind::Minidump))
            || pending.demand.payload_groups() != 1
            || pending
                .payloads
                .iter()
                .try_fold(bytes, |sum, (_, size, ..)| sum.checked_add(*size))
                .is_none_or(|sum| sum > pending.demand.payload_bytes())
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        if let Some(copy) = &*retained {
            if copy.ticket != ticket.id || copy.bytes != bytes {
                return Err(IncidentStoreError::RecoveryConflict);
            }
            if let Some(facts) = copy.copied {
                return Ok(facts);
            }
        } else {
            self.inventory(&original)?;
            let stage = self.stages.len();
            self.stages.push(self.storage.stage(object, bytes, &self.operation, &original)?)?;
            *retained = Some(CopyState {
                ticket: ticket.id,
                stage,
                bytes,
                offset: 0,
                sha256: Sha256::new(),
                copied: None,
                write_uncertain: false,
                activation_started: false,
            });
        }
        let copy = retained.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        if copy.write_uncertain {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        // This actual fixed stack buffer is included in the finished Rust role inequality.
        let mut chunk = [0; super::super::native::finished_report::COPY_CHUNK_BYTES];
        while copy.offset < copy.bytes {
            let capacity = usize::try_from((copy.bytes - copy.offset).min(chunk.len() as u64))
                .map_err(|_| IncidentStoreError::Capacity)?;
            let read = token
                .read(copy.offset, &mut chunk[..capacity], &original)
                .map_err(|_| IncidentStoreError::RecoveryConflict)?;
            if read != capacity {
                return Err(IncidentStoreError::MissingPayload);
            }
            copy.write_uncertain = true;
            self.stages.get_mut(copy.stage)?.write_chunk(&chunk[..read], &original)?;
            copy.sha256.update(&chunk[..read]);
            copy.offset += read as u64;
            copy.write_uncertain = false;
        }
        let expected: [u8; 32] = copy.sha256.clone().finalize().into();
        let flushed = self.stages.get_mut(copy.stage)?.flush(&original)?.read_facts();
        if flushed.bytes() != copy.bytes || flushed.sha256() != expected {
            return Err(IncidentStoreError::WrongDigest);
        }
        let actual = if copy.activation_started {
            self.storage.reconcile_activation(
                object,
                super::repository::predecessor(flushed),
                &self.operation,
                &original,
            )?
        } else {
            copy.activation_started = true;
            self.storage.activate(
                self.stages.get_mut(copy.stage)?,
                DiagnosticPredecessor::Absent,
                &self.operation,
                &original,
            )?
        };
        self.stages.remove(copy.stage)?;
        let facts = AttachmentFacts::new(AttachmentKind::Minidump, bytes, bytes, 0, expected)?;
        let pending = self.pending.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        pending.payloads.push((AttachmentKind::Minidump, bytes, bytes, 0, actual));
        copy.copied = Some(facts);
        Ok(facts)
    }
}
