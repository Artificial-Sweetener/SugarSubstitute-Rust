// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep physical-copy generation changes in the external passive source boundary.
use super::*;
use crate::incident::portable::*;
use std::{cell::Cell, io, rc::Rc};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_supervision::diagnostic_storage::{DiagnosticEntryClass, DiagnosticObject};
pub(super) type TestError = Box<dyn Error + Send + Sync>;
pub(super) type TestResult = Result<(), TestError>;

#[derive(Clone)]
pub(super) struct Row {
    pub object: Option<DiagnosticObject>,
    pub class: DiagnosticEntryClass,
    pub bytes: Vec<u8>,
    pub physical: u64,
}
pub(super) struct Source {
    pub rows: Vec<Row>,
    entries: Vec<PortableIncidentEntry>,
    provenance: PortableIncidentProvenance,
    pinned: Vec<u64>,
    pub generation: Rc<Cell<u64>>,
    pub reads: usize,
    pub short: bool,
}
impl Source {
    pub(super) fn new() -> Result<Self, TestError> {
        let namespace = NamespaceId::new([1; 32]);
        let ledger = SummaryEvictions::new(namespace, 0, 0, 0, None)?;
        let mut source = Self {
            rows: Vec::new(),
            entries: Vec::new(),
            provenance: PortableIncidentProvenance::new(namespace, [7; 32])?,
            pinned: Vec::new(),
            generation: Rc::new(Cell::new(1)),
            reads: 0,
            short: false,
        };
        source.add(
            DiagnosticEntryClass::StoreState,
            Some(DiagnosticObject::StoreState),
            encode_store_state(&IncidentStoreState::Ledger(ledger))?,
        )?;
        Ok(source)
    }
    pub(super) fn add(
        &mut self,
        class: DiagnosticEntryClass,
        object: Option<DiagnosticObject>,
        bytes: Vec<u8>,
    ) -> Result<(), TestError> {
        let id = PortableIncidentEntryId::new(u16::try_from(self.rows.len())?)?;
        self.entries.push(PortableIncidentEntry::new(id, class, bytes.len() as u64)?);
        let physical = self.rows.len() as u64 + 1;
        self.rows.push(Row { object, class, bytes, physical });
        self.pinned.push(physical);
        Ok(())
    }
    pub(super) fn copied(mut self) -> Self {
        for row in &mut self.rows {
            row.physical += 1000;
        }
        self.pinned = self.rows.iter().map(|row| row.physical).collect();
        self
    }
    pub(super) fn unchanged(&self, bytes: &[Vec<u8>]) -> bool {
        self.rows.iter().map(|row| &row.bytes).eq(bytes.iter())
    }
    pub(super) fn original_bytes(&self) -> Vec<Vec<u8>> {
        self.rows.iter().map(|row| row.bytes.clone()).collect()
    }
    pub(super) fn duplicate_inventory(&mut self) {
        self.entries.push(self.entries[0]);
    }
    pub(super) fn summary(&mut self, stored: StoredIncidentSummary) -> Result<(), TestError> {
        let summary = stored.summary();
        self.add(
            DiagnosticEntryClass::Seed,
            Some(DiagnosticObject::Seed(summary.seed().key())),
            encode_seed(summary.seed())?,
        )?;
        self.add(
            DiagnosticEntryClass::Summary,
            Some(DiagnosticObject::Summary(summary.seed().key())),
            encode_summary(&stored)?,
        )
    }
    pub(super) fn retained(&mut self, bytes: &[u8]) -> Result<IncidentKey, TestError> {
        let summary = super::reporting_fixture::retained(bytes)?;
        let key = summary.seed().key();
        let revision = summary.revision();
        self.summary(StoredIncidentSummary::new(summary, Some(revision))?)?;
        self.add(
            DiagnosticEntryClass::Payload,
            Some(DiagnosticObject::Payload(key, revision, AttachmentKind::Fault)),
            bytes.to_vec(),
        )?;
        Ok(key)
    }
}
impl PortableIncidentReadSession for Source {
    fn provenance(&self) -> PortableIncidentProvenance {
        self.provenance
    }
    fn inventory(
        &mut self,
        _: &JobContext,
    ) -> Result<&[PortableIncidentEntry], PortableIncidentError> {
        Ok(&self.entries)
    }
    fn read_chunk(
        &mut self,
        entry: PortableIncidentEntryId,
        offset: u64,
        output: &mut [u8],
        _: &JobContext,
    ) -> Result<usize, PortableIncidentError> {
        self.reads += 1;
        let row = self.rows.get(usize::from(entry.get())).ok_or_else(|| {
            PortableIncidentError::rejected(PortableIncidentFailureCode::ChangedSource)
        })?;
        let offset = usize::try_from(offset)
            .map_err(|cause| PortableIncidentError::source(io::Error::other(cause)))?;
        if self.short || offset >= row.bytes.len() {
            return Ok(0);
        }
        let count = output.len().min(row.bytes.len() - offset).min(37);
        output[..count].copy_from_slice(&row.bytes[offset..offset + count]);
        Ok(count)
    }
    fn bind_typed_object(
        &mut self,
        entry: PortableIncidentEntryId,
        object: DiagnosticObject,
        _: &JobContext,
    ) -> Result<bool, PortableIncidentError> {
        Ok(self.rows.get(usize::from(entry.get())).is_some_and(|row| row.object == Some(object)))
    }
    fn revalidate(&mut self, _: &JobContext) -> Result<(), PortableIncidentError> {
        if self.generation.get() != 1
            || self.rows.iter().map(|row| row.physical).ne(self.pinned.iter().copied())
        {
            return Err(PortableIncidentError::rejected(
                PortableIncidentFailureCode::ChangedSource,
            ));
        }
        Ok(())
    }
}
pub(super) fn admitted(work: impl FnOnce(JobContext) -> TestResult + Send + 'static) -> TestResult {
    super::publication_fixture::admitted_bytes(32 * 1024 * 1024, work)
}
pub(super) fn now() -> Result<IncidentUtc, TestError> {
    Ok(IncidentUtc::from_unix_seconds(111)?)
}
pub(super) fn base_summary() -> Result<StoredIncidentSummary, TestError> {
    let summary = super::summary().map_err(|cause| io::Error::other(cause.to_string()))?;
    Ok(StoredIncidentSummary::new(summary, None)?)
}
