// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Run the real group transaction with a closed codec boundary and external disk.
mod cancellation;
mod custody;
mod deadline;
mod fatal;
mod lifecycle;
mod limits;
mod recovery;
use super::*;
use crate::diagnostic_storage::fixture::DiagnosticFixtureFailure;
use sha2::{Digest, Sha256};
const ORIGINAL: &[u8] = b"original schema2 retained summary";
const CANDIDATE: &[u8] = b"candidate schema2 discarded summary";
const JOURNAL: &[u8] = b"strict payload discard journal";
const PAYLOADS: [&[u8]; 2] = [b"log payload", b"fault payload"];
const KINDS: [AttachmentKind; 2] = [AttachmentKind::Log, AttachmentKind::Fault];
struct Validator {
    selection: DiagnosticPayloadDiscardSelection,
    historical: LogicalStoreOperationKey,
    original: RecordedDiagnosticFileFacts,
    candidate: RecordedDiagnosticFileFacts,
    victims: Vec<DiagnosticPayloadVictimExpectation>,
    hook: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}
impl Validator {
    fn decision(
        &self,
        allocation: DiagnosticPayloadDecisionAllocationLoan,
        case: DiagnosticPayloadDiscardCase,
    ) -> Result<DiagnosticPayloadDiscardDecision, DiagnosticRecoveryValidationError> {
        DiagnosticPayloadDiscardDecision::new(
            allocation,
            self.selection.clone(),
            self.historical,
            self.original,
            self.candidate,
            &self.victims,
            Sha256::digest(JOURNAL).into(),
            case,
        )
    }
    fn invoke(&self) {
        let hook = self.hook.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
        if let Some(hook) = hook {
            hook();
        }
    }
}
impl DiagnosticPayloadDiscardValidator for Validator {
    fn validate_preparation(
        &self,
        _decode: &mut DiagnosticRecoveryDecodeLoan,
        allocation: DiagnosticPayloadDecisionAllocationLoan,
        current: LogicalStoreOperationKey,
        selection: &DiagnosticPayloadDiscardSelection,
        journal: &[u8],
        original: &DiagnosticRecoverySource,
        candidate: &DiagnosticRecoverySource,
        victims: &[DiagnosticPayloadVictimObservation],
    ) -> Result<DiagnosticPayloadDiscardDecision, DiagnosticRecoveryValidationError> {
        if current != self.historical
            || selection != &self.selection
            || journal != JOURNAL
            || original.as_bytes() != ORIGINAL
            || !self.original.matches(original.facts())
            || candidate.as_bytes() != CANDIDATE
            || !self.candidate.matches(candidate.facts())
            || candidate.class() != DiagnosticEntryClass::Stage
            || victims.len() != self.victims.len()
            || victims.iter().zip(&self.victims).any(|(actual, expected)| {
                recorded(actual.facts()).ok().and_then(|facts| {
                    DiagnosticPayloadVictimExpectation::new(actual.object(), facts).ok()
                }) != Some(*expected)
            })
        {
            return Err(DiagnosticRecoveryValidationError::rejected(
                DiagnosticRecoveryValidationCode::InvalidIdentity,
            ));
        }
        self.invoke();
        self.decision(allocation, DiagnosticPayloadDiscardCase::Original)
    }
    fn validate_recovery(
        &self,
        _decode: &mut DiagnosticRecoveryDecodeLoan,
        allocation: DiagnosticPayloadDecisionAllocationLoan,
        _: LogicalStoreOperationKey,
        journal: &DiagnosticRecoverySource,
        summary: &DiagnosticRecoverySource,
    ) -> Result<DiagnosticPayloadDiscardDecision, DiagnosticRecoveryValidationError> {
        if journal.as_bytes() != JOURNAL
            || journal.class() != DiagnosticEntryClass::Journal
            || summary.class() != DiagnosticEntryClass::Summary
        {
            return Err(DiagnosticRecoveryValidationError::rejected(
                DiagnosticRecoveryValidationCode::InvalidJournal,
            ));
        }
        let case = if self.original.matches(summary.facts()) && summary.as_bytes() == ORIGINAL {
            DiagnosticPayloadDiscardCase::Original
        } else if self.candidate.matches(summary.facts()) && summary.as_bytes() == CANDIDATE {
            DiagnosticPayloadDiscardCase::Candidate
        } else {
            return Err(DiagnosticRecoveryValidationError::rejected(
                DiagnosticRecoveryValidationCode::InvalidIdentity,
            ));
        };
        self.invoke();
        self.decision(allocation, case)
    }
}
fn recorded(
    facts: DiagnosticReadFacts,
) -> Result<RecordedDiagnosticFileFacts, DiagnosticStorageError> {
    RecordedDiagnosticFileFacts::new(facts.identity, facts.sha256, facts.bytes)
}
fn payload(index: usize) -> Result<DiagnosticObject, TestError> {
    Ok(DiagnosticObject::Payload(incident()?, IncidentRevision::new(1)?, KINDS[index]))
}
fn selection() -> Result<DiagnosticPayloadDiscardSelection, TestError> {
    let facts = KINDS
        .iter()
        .zip(PAYLOADS)
        .map(|(kind, bytes)| {
            AttachmentFacts::new(
                *kind,
                bytes.len() as u64,
                bytes.len() as u64,
                0,
                Sha256::digest(bytes).into(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let inventory = AttachmentInventory::new(&facts)?;
    Ok(DiagnosticPayloadDiscardSelection::new(
        incident()?,
        IncidentRevision::new(1)?,
        IncidentRevision::new(1)?,
        inventory.clone(),
        DiscardedPayload::new(PayloadDiscard::StoragePressure, inventory.total_bytes())?,
    )?)
}
fn disk() -> Result<Filesystem, TestError> {
    let disk = Filesystem::new();
    disk.put(DiagnosticObject::Summary(incident()?), ORIGINAL);
    for (index, bytes) in PAYLOADS.iter().enumerate() {
        disk.put(payload(index)?, bytes);
    }
    Ok(disk)
}
fn prepare(
    storage: &mut DiagnosticStorage,
    operation: &DiagnosticStorageOperation,
    context: &JobContext,
) -> Result<
    (DiagnosticPayloadDiscardPreparation, Validator, DiagnosticReadLease, DiagnosticStage),
    TestError,
> {
    let mut original =
        storage.read(DiagnosticObject::Summary(incident()?), 64, operation, context)?;
    let mut candidate =
        storage.stage(DiagnosticObject::Summary(incident()?), 64, operation, context)?;
    candidate.write_chunk(CANDIDATE, context)?;
    candidate.flush(context)?;
    let preparation = storage.prepare_payload_discard(
        &mut original,
        &mut candidate,
        selection()?,
        operation,
        context,
    )?;
    let victims = preparation
        .victims()
        .map(|actual| {
            Ok(DiagnosticPayloadVictimExpectation::new(actual.object(), recorded(actual.facts())?)?)
        })
        .collect::<Result<Vec<_>, TestError>>()?;
    let validator = Validator {
        selection: selection()?,
        historical: operation.key,
        original: recorded(preparation.original_file())?,
        candidate: recorded(preparation.candidate_stage())?,
        victims,
        hook: Mutex::new(None),
    };
    Ok((preparation, validator, original, candidate))
}
fn normal(
    storage: &mut DiagnosticStorage,
    operation: &DiagnosticStorageOperation,
    context: &JobContext,
) -> Result<DiagnosticPayloadDiscardPermit, TestError> {
    let (mut preparation, validator, _, _) = prepare(storage, operation, context)?;
    let mut journal =
        storage.seal_payload_discard(&mut preparation, JOURNAL, &validator, operation, context)?;
    Ok(storage.activate_payload_discard(&mut preparation, &mut journal, operation, context)?)
}
fn recovered_disk(
    candidate: bool,
    missing: Option<usize>,
) -> Result<(Filesystem, Validator), TestError> {
    let disk = Filesystem::new();
    let original = disk.put(DiagnosticObject::Summary(incident()?), ORIGINAL);
    let candidate_facts = if candidate {
        disk.erase_existing(DiagnosticObject::Summary(incident()?));
        disk.put(DiagnosticObject::Summary(incident()?), CANDIDATE)
    } else {
        disk.put_stage(DiagnosticObject::Summary(incident()?), key(17)?, CANDIDATE)
    };
    let mut victims = Vec::new();
    for (index, bytes) in PAYLOADS.iter().enumerate() {
        let facts = disk.put(payload(index)?, bytes);
        victims.push(DiagnosticPayloadVictimExpectation::new(payload(index)?, recorded(facts)?)?);
        if missing == Some(index) {
            disk.erase_existing(payload(index)?);
        }
    }
    disk.put(DiagnosticObject::Journal(key(17)?), JOURNAL);
    let validator = Validator {
        selection: selection()?,
        historical: key(17)?,
        original: recorded(original)?,
        candidate: recorded(candidate_facts)?,
        victims,
        hook: Mutex::new(None),
    };
    Ok((disk, validator))
}
fn recover(
    storage: &mut DiagnosticStorage,
    operation: &DiagnosticStorageOperation,
    context: &JobContext,
    validator: &Validator,
) -> Result<DiagnosticPayloadDiscardPermit, TestError> {
    let mut journal =
        storage.read(DiagnosticObject::Journal(validator.historical), 64, operation, context)?;
    let mut summary =
        storage.read(DiagnosticObject::Summary(incident()?), 64, operation, context)?;
    Ok(storage.recover_payload_discard(
        &mut journal,
        &mut summary,
        validator,
        operation,
        context,
    )?)
}
