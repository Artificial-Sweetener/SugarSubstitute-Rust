// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate protected committed dumps while retaining exact reader and decode custody.
pub(crate) mod cause;
mod read;
pub(in crate::incident) mod validation;
use super::{IncidentRepository, StoredIncidentSummary};
use cause::DumpReadCause;
pub use cause::{DumpReadCode, DumpReadFailure};
use sugarsubstitute_contracts::{
    execution::{MemoryClaim, MemoryClaimError},
    incident::*,
};
use sugarsubstitute_execution::{CommittedReaderOrigin, CommittedReaderRetirement};
use sugarsubstitute_supervision::committed_reader::{CommittedDiagnosticReader, CommittedReadSlot};
/// Retain the one original reader, exact decoded record and source/parser allocation.
/// Construction allocates only after claiming its fixed decode region from actual Main.
/// All effects run on the finite execution worker; this owner has no scheduling API.
pub struct IncidentDumpReader {
    fields: Box<Fields>,
    native: Option<CommittedDiagnosticReader>,
    _decode: MemoryClaim,
}
struct Fields {
    source: Vec<u8>,
    stored: Option<StoredIncidentSummary>,
    ledger: Option<SummaryEvictions>,
    summary_slot: Option<CommittedReadSlot>,
    dump_slot: Option<CommittedReadSlot>,
    first: Option<DumpReadCause>,
    secondary: Option<DumpReadCause>,
    retirement: Option<DumpReadCause>,
    parser: Option<crate::native::committed_parser::CommittedDumpIdentity>,
    verified: bool,
    checked_current: bool,
}
/// Return the unchanged native reader when the original decode region cannot be admitted.
pub struct DumpReaderConstructionFailure {
    /// Original execution allocation rejection; it drops before original reader custody.
    pub cause: DumpReaderConstructionCause,
    /// Same genuine reader admission; no file has been opened by this constructor.
    pub reader: CommittedDiagnosticReader,
}
/// Genuine construction failure; neither branch manufactures a replacement claim.
pub enum DumpReaderConstructionCause {
    /// Original Core owner was unavailable.
    Control(sugarsubstitute_contracts::execution::AdmissionError),
    /// Original job could not admit the fixed decoder destination.
    Allocation(MemoryClaimError),
}
impl IncidentDumpReader {
    /// Admit768KiB from the same original Main reservation before Box, Vec or decoding.
    /// This is separate from the producer's real3MiB reader share; no new pool is created.
    /// # Errors
    /// Returns identical native custody and the genuine original allocation error.
    pub fn new(reader: CommittedDiagnosticReader) -> Result<Self, DumpReaderConstructionFailure> {
        if let Err(cause) = reader.check_effect() {
            return Err(DumpReaderConstructionFailure {
                reader,
                cause: DumpReaderConstructionCause::Control(cause),
            });
        }
        let result = reader
            .original_origin()
            .original_context()
            .map(|context| context.memory().claim(786_432));
        // A successfully constructed reader necessarily retains its original context.
        let claim = match result {
            Ok(Ok(claim)) => claim,
            Ok(Err(cause)) => {
                return Err(DumpReaderConstructionFailure {
                    reader,
                    cause: DumpReaderConstructionCause::Allocation(cause),
                });
            }
            Err(cause) => {
                return Err(DumpReaderConstructionFailure {
                    reader,
                    cause: DumpReaderConstructionCause::Control(cause),
                });
            }
        };
        Ok(Self {
            fields: Box::new(Fields {
                source: Vec::new(),
                stored: None,
                ledger: None,
                summary_slot: None,
                dump_slot: None,
                first: None,
                secondary: None,
                retirement: None,
                parser: None,
                verified: false,
                checked_current: false,
            }),
            native: Some(reader),
            _decode: claim,
        })
    }
    /// Return the actual separately admitted decoder/source region in bytes.
    pub const fn decode_bytes(&self) -> u64 {
        self._decode.bytes()
    }
    /// Borrow the genuine original paired target for close-only cleanup.
    pub fn original_origin(&self) -> Option<&CommittedReaderOrigin> {
        self.native.as_ref().map(CommittedDiagnosticReader::original_origin)
    }
    /// Observe the first actual source, parser or Core failure with its genuine cause.
    pub fn failure(&self) -> Option<DumpReadFailure<'_>> {
        self.fields.first.as_ref().map(DumpReadFailure)
    }
    /// Borrow additional post-read or checked-retirement failure without replacing the first.
    pub fn secondary_failure(&self) -> Option<DumpReadFailure<'_>> {
        self.fields.secondary.as_ref().map(DumpReadFailure)
    }
    /// Borrow a checked-retirement failure without displacing source or post-read causes.
    pub fn retirement_failure(&self) -> Option<DumpReadFailure<'_>> {
        self.fields.retirement.as_ref().map(DumpReadFailure)
    }
    /// Transfer the exact fully validated reader into an opaque committed dump capability.
    /// # Errors
    /// Returns unchanged owner until parsing and actual current-summary observation succeed.
    pub fn take_verified(self) -> Result<IncidentDumpReadLease, Self> {
        if self.fields.verified && self.fields.checked_current && self.fields.first.is_none() {
            Ok(IncidentDumpReadLease { owner: self })
        } else {
            Err(self)
        }
    }
    pub(crate) fn record(&mut self, cause: DumpReadCause) -> DumpReadCode {
        let code = cause.code();
        if self.fields.first.is_none() {
            self.fields.first = Some(cause);
        } else if self.fields.secondary.is_none() {
            self.fields.secondary = Some(cause);
        }
        self.fields.first.as_ref().map_or(code, DumpReadCause::code)
    }
    /// Checked-close the same files/fence under genuine paired close-only authority.
    /// # Errors
    /// An uncertain close returns this identical owner with all decode/native custody.
    pub fn retire(mut self, proof: &CommittedReaderRetirement) -> Result<(), Self> {
        if self.retire_in_place(proof).is_err() {
            return Err(self);
        }
        Ok(())
    }
    /// Checked-close native pins while retaining decoded records and original causes.
    /// # Errors
    /// Uncertain closure remains owned here; returned category is inert.
    pub fn retire_in_place(
        &mut self,
        proof: &CommittedReaderRetirement,
    ) -> Result<(), DumpReadCode> {
        let Some(native) = self.native.take() else {
            return Ok(());
        };
        match native.retire(proof) {
            Ok(()) => Ok(()),
            Err(failure) => {
                self.native = Some(failure.owner);
                let cause = DumpReadCause::Native(failure.cause);
                let code = cause.code();
                if self.fields.retirement.is_none() {
                    self.fields.retirement = Some(cause);
                }
                Err(code)
            }
        }
    }
}
/// A noncloneable validated same-inode dump capability; no paths or handles escape.
/// Its original source/decode charge survives Main completion until actual allocation Drop.
pub struct IncidentDumpReadLease {
    owner: IncidentDumpReader,
}
impl IncidentDumpReadLease {
    /// Borrow exact schema2 summary, including immutable payload publication revision.
    pub fn summary(&self) -> Result<&StoredIncidentSummary, DumpReadCode> {
        self.owner.fields.stored.as_ref().ok_or(DumpReadCode::Identity)
    }
    /// Borrow actual successful module/UUID observations from the same copied-inode parser.
    /// # Errors
    /// Rejects an unavailable parser observation without fabricating matching facts.
    pub fn parser_identity(
        &self,
    ) -> Result<crate::native::committed_parser::CommittedDumpIdentity, DumpReadCode> {
        self.owner.fields.parser.ok_or(DumpReadCode::Identity)
    }
    /// Return expected full incident key after actual protected decoding and parser validation.
    pub fn key(&self) -> Result<IncidentKey, DumpReadCode> {
        Ok(self.summary()?.summary().seed().key())
    }
    /// Return current summary revision independently of immutable payload revision.
    pub fn revision(&self) -> Result<IncidentRevision, DumpReadCode> {
        Ok(self.summary()?.summary().revision())
    }
    /// Return the actual selected finished Crashpad UUID.
    pub fn dump(&self) -> Result<DumpId, DumpReadCode> {
        match self.summary()?.summary().facts().capture() {
            CaptureEvidence::Finished(dump) => Ok(dump),
            _ => Err(DumpReadCode::Identity),
        }
    }
    /// Borrow the original Core target for checked cleanup.
    pub fn original_origin(&self) -> Option<&CommittedReaderOrigin> {
        self.owner.original_origin()
    }
    /// Read at most64KiB from the exact copied dump under original running Main controls.
    /// # Errors
    /// Genuine source failures remain retained by this owner and block further success.
    pub fn read_chunk(&mut self, offset: u64, output: &mut [u8]) -> Result<usize, DumpReadCode> {
        let result = self.owner.read_dump(offset, output);
        result.map_err(|cause| self.owner.record(cause))
    }
    /// Borrow actual same-file size/hash/identity, never native access authority.
    /// # Errors
    /// Returns categorical failure if a retained slot is unavailable; cause remains owned.
    pub fn facts(
        &mut self,
    ) -> Result<sugarsubstitute_supervision::diagnostic_storage::DiagnosticReadFacts, DumpReadCode>
    {
        let result =
            self.owner.fields.dump_slot.as_ref().ok_or(DumpReadCause::Identity).and_then(|slot| {
                self.owner
                    .native
                    .as_ref()
                    .ok_or(DumpReadCause::Identity)?
                    .facts(slot)
                    .map_err(DumpReadCause::Native)
            });
        result.map_err(|cause| self.owner.record(cause))
    }
    /// Repeat actual Summary re-observation while Main is Running and all pins remain held.
    /// # Errors
    /// Rejects changed native facts and retains their genuine cause.
    pub fn check_current(&mut self) -> Result<(), DumpReadCode> {
        self.owner.check_current()
    }
    /// Checked-close all actual pins/fence, returning identical lease on uncertainty.
    /// # Errors
    /// Failure retains the same original controls, record, sources and allocations.
    pub fn retire(self, proof: &CommittedReaderRetirement) -> Result<(), Self> {
        self.owner.retire(proof).map_err(|owner| Self { owner })
    }
    /// Checked-close pins while retaining the exact original decoder and error lifetime.
    /// # Errors
    /// Original closure failure stays in this same lease.
    pub fn retire_in_place(
        &mut self,
        proof: &CommittedReaderRetirement,
    ) -> Result<(), DumpReadCode> {
        self.owner.retire_in_place(proof)
    }
}
impl IncidentRepository {
    /// Validate a committed dump through the repository's sole strict record owners.
    /// The actual readonly producer supplies existing-only native authority; this
    /// method never creates a writer, migrates a record or infers ledger absence.
    /// Injected UTC and expected matching identities cannot grant file access.
    /// # Errors
    /// Original failures remain in `reader`; the returned category is inert.
    pub fn open_dump(
        reader: &mut IncidentDumpReader,
        key: IncidentKey,
        revision: IncidentRevision,
        dump: DumpId,
        utc: IncidentUtc,
    ) -> Result<(), DumpReadCode> {
        let result = reader.validate(key, revision, dump, utc);
        result.map_err(|cause| reader.record(cause))
    }
}

const _: () = assert!(size_of::<Fields>() < 8192);

impl std::fmt::Debug for IncidentDumpReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("IncidentDumpReader")
    }
}
impl std::fmt::Display for IncidentDumpReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Committed dump retained failure")
    }
}
impl std::error::Error for IncidentDumpReader {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.fields
            .first
            .as_ref()
            .or(self.fields.retirement.as_ref())
            .map(|cause| cause as &(dyn std::error::Error + 'static))
    }
}
impl std::fmt::Debug for IncidentDumpReadLease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("IncidentDumpReadLease")
    }
}
impl std::fmt::Display for IncidentDumpReadLease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.owner.fmt(f)
    }
}
impl std::error::Error for IncidentDumpReadLease {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.owner)
    }
}
