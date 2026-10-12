// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Define the sole private protected-filesystem primitive boundary and closed stage access.
use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SummaryStageAccess {
    PreservedOriginal,
    Candidate,
}

pub(crate) trait Port: Send {
    fn bind_operation(&mut self, context: &JobContext);
    fn resolve(
        &mut self,
        root: &Path,
        namespace: NamespaceId,
        context: &JobContext,
    ) -> Result<(), ClaimError>;
    fn acquire_writer(&mut self, context: &JobContext) -> Result<(), ClaimError>;
    fn scan(&mut self, context: &JobContext) -> Result<inventory::Scan, ClaimError>;
    fn open(
        &mut self,
        object: DiagnosticObject,
        bound: u64,
        context: &JobContext,
    ) -> Result<(NativeId, DiagnosticReadFacts), ClaimError>;
    fn open_inventory(
        &mut self,
        observed: &inventory::ObservedEntry,
        bound: u64,
        context: &JobContext,
    ) -> Result<(NativeId, DiagnosticReadFacts), ClaimError>;
    fn create_stage(
        &mut self,
        object: DiagnosticObject,
        operation: LogicalStoreOperationKey,
        reserved: u64,
        context: &JobContext,
    ) -> Result<NativeId, ClaimError>;
    fn recover_stage(
        &mut self,
        object: DiagnosticObject,
        historical: LogicalStoreOperationKey,
        expected: RecordedDiagnosticFileFacts,
        context: &JobContext,
    ) -> Result<(NativeId, DiagnosticReadFacts), ClaimError>;
    fn recover_summary_stage(
        &mut self,
        object: DiagnosticObject,
        historical: LogicalStoreOperationKey,
        expected: RecordedDiagnosticFileFacts,
        access: SummaryStageAccess,
        context: &JobContext,
    ) -> Result<(NativeId, DiagnosticReadFacts), ClaimError>;
    fn observe_file(
        &mut self,
        id: NativeId,
        context: &JobContext,
    ) -> Result<DiagnosticReadFacts, ClaimError>;
    fn stage_target(
        &mut self,
        id: NativeId,
        object: DiagnosticObject,
        context: &JobContext,
    ) -> Result<Option<DiagnosticReadFacts>, ClaimError>;
    fn read(
        &mut self,
        id: NativeId,
        offset: u64,
        output: &mut [u8],
        context: &JobContext,
    ) -> Result<usize, ClaimError>;
    fn write(
        &mut self,
        id: NativeId,
        offset: u64,
        input: &[u8],
        context: &JobContext,
    ) -> Result<(), ClaimError>;
    fn flush(
        &mut self,
        id: NativeId,
        context: &JobContext,
    ) -> Result<DiagnosticReadFacts, ClaimError>;
    fn activate(
        &mut self,
        id: NativeId,
        object: DiagnosticObject,
        expected: DiagnosticPredecessor,
        context: &JobContext,
    ) -> Result<DiagnosticReadFacts, ClaimError>;
    fn reconcile_activation(
        &mut self,
        object: DiagnosticObject,
        expected: DiagnosticPredecessor,
        context: &JobContext,
    ) -> Result<DiagnosticReadFacts, ClaimError>;
    fn delete(
        &mut self,
        id: NativeId,
        object: DiagnosticObject,
        expected: DiagnosticReadFacts,
        context: &JobContext,
    ) -> Result<(), ClaimError>;
    fn absent(
        &mut self,
        object: DiagnosticObject,
        context: &JobContext,
    ) -> Result<bool, ClaimError>;
    fn close(&mut self, id: NativeId, context: &JobContext) -> Result<(), ClaimError>;
    fn retire(&mut self, context: &JobContext) -> Result<(), ClaimError>;
    fn live_handles(&self) -> usize;
    fn retained_files(&self) -> usize;
}
