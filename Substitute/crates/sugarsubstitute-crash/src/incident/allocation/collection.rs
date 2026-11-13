// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep both literal collection vectors with one original collection phase.
use super::{
    super::{IncidentStoreError, StoredIncidentSummary},
    RetainedSummary,
};
use std::ops::Deref;
use sugarsubstitute_contracts::{
    execution::JobContext,
    incident::{IncidentKey, MAX_SUMMARIES},
};
use sugarsubstitute_supervision::diagnostic_storage::{
    DiagnosticReadFacts, DiagnosticRecoveryCollectionLoan,
};

pub(in crate::incident) struct SummaryCollection {
    pub(in crate::incident) keys: Vec<IncidentKey>,
    pub(in crate::incident) records: Vec<(StoredIncidentSummary, DiagnosticReadFacts)>,
    custody: DiagnosticRecoveryCollectionLoan,
}
impl SummaryCollection {
    pub(in crate::incident) fn new(
        custody: DiagnosticRecoveryCollectionLoan,
    ) -> Result<Self, IncidentStoreError> {
        let mut collection = Self { keys: Vec::new(), records: Vec::new(), custody };
        if let Err(source) = collection.keys.try_reserve_exact(MAX_SUMMARIES) {
            return Err(collection.allocation_failed(source));
        }
        if let Err(source) = collection.records.try_reserve_exact(MAX_SUMMARIES) {
            return Err(collection.allocation_failed(source));
        }
        if collection.keys.capacity() != MAX_SUMMARIES
            || collection.records.capacity() != MAX_SUMMARIES
        {
            return Err(IncidentStoreError::Capacity);
        }
        Ok(collection)
    }
    pub(in crate::incident) fn allocation_failed(
        self,
        source: std::collections::TryReserveError,
    ) -> IncidentStoreError {
        let Self { keys, records, custody } = self;
        drop(keys);
        drop(records);
        IncidentStoreError::Allocation(super::LoanedAllocationFailure::collection(source, custody))
    }
    pub(in crate::incident) fn transfer(
        mut self,
        index: usize,
        context: &JobContext,
    ) -> Result<(RetainedSummary, DiagnosticReadFacts), IncidentStoreError> {
        if index >= self.records.len() {
            return Err(IncidentStoreError::Capacity);
        }
        let index_byte = u8::try_from(index).map_err(|_| IncidentStoreError::Capacity)?;
        let custody = self.custody.transfer_record(index_byte, context)?;
        let (stored, facts) = self.records.remove(index);
        Ok((RetainedSummary::new(stored, custody), facts))
    }
}
impl Deref for SummaryCollection {
    type Target = [(StoredIncidentSummary, DiagnosticReadFacts)];
    fn deref(&self) -> &Self::Target {
        &self.records
    }
}
