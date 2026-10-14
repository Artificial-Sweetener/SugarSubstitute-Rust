// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep actual canonical read custody through physical capture disposition and journal retirement.
use super::*;
mod fence;
pub(in crate::diagnostic_storage) mod retirement;
pub(in crate::diagnostic_storage) mod types;
pub use fence::DiagnosticCaptureFence;
pub use types::*;
#[cfg(test)]
mod tests;

pub(super) struct CapturePin {
    id: u64,
    native: NativeId,
    object: DiagnosticObject,
    facts: DiagnosticReadFacts,
}
pub(super) struct RetainedCapture {
    id: u64,
    journal: CapturePin,
    summary: CapturePin,
    payload: Option<CapturePin>,
    callback_active: bool,
    native_settled: bool,
    delete_attempted: bool,
    delete_issued: bool,
    journal_closed: bool,
}
impl RetainedCapture {
    fn pins(&self, id: u64) -> bool {
        self.journal.id == id
            || self.summary.id == id
            || self.payload.as_ref().is_some_and(|p| p.id == id)
    }
}
pub(super) fn pins(state: &State, id: u64) -> bool {
    state.capture.as_ref().is_some_and(|intent| intent.pins(id))
}
pub(super) fn retained(
    state: &State,
    id: u64,
    key: LogicalStoreOperationKey,
) -> Result<&RetainedCapture, DiagnosticStorageError> {
    state
        .capture
        .as_ref()
        .filter(|intent| intent.id == id)
        .ok_or_else(|| error(key, Code::InvalidIdentity))
}
pub(super) fn retained_mut(
    state: &mut State,
    id: u64,
    key: LogicalStoreOperationKey,
) -> Result<&mut RetainedCapture, DiagnosticStorageError> {
    state
        .capture
        .as_mut()
        .filter(|intent| intent.id == id)
        .ok_or_else(|| error(key, Code::InvalidIdentity))
}
impl DiagnosticStorage {
    /// Pin actual canonical read slots before the native disposition adapter can run.
    /// Optional payload is accepted only when the native adapter validates a discarded Summary.
    /// Dropping the returned intent preserves its storage-owned fence and native custody.
    /// # Errors
    /// Rejects foreign/retired reads, unrelated objects, missing writer/source/CPP owner,
    /// conflicting intent or pending publication stage. No native effect occurs on rejection.
    pub fn adopt_capture_intent(
        &mut self,
        journal: &mut DiagnosticReadLease,
        summary: &mut DiagnosticReadLease,
        payload: Option<&mut DiagnosticReadLease>,
        operation: &DiagnosticStorageOperation,
        context: &JobContext,
    ) -> Result<DiagnosticCaptureIntent, DiagnosticStorageError> {
        let mut state = self.check(operation, context)?;
        let data = state.data_mut()?;
        if !data.writer
            || data.capture.is_some()
            || data.deletion.is_some()
            || data.migration.is_some()
            || data.payload.is_some()
            || !data.stages.is_empty()
            || data.capture_reads != 1
            || data.finished_owner.is_none()
        {
            return Err(error(operation.key, Code::Busy));
        }
        let journal = pin(data, &self.shared, journal, operation.key)?;
        let summary = pin(data, &self.shared, summary, operation.key)?;
        let payload =
            payload.map(|read| pin(data, &self.shared, read, operation.key)).transpose()?;
        let DiagnosticObject::Journal(historical) = journal.object else {
            return Err(error(operation.key, Code::InvalidIdentity));
        };
        let DiagnosticObject::Summary(key) = summary.object else {
            return Err(error(operation.key, Code::InvalidIdentity));
        };
        if historical.owner().namespace != operation.key.owner().namespace || key.subject().coordinator() != historical.owner()
            || journal.id == summary.id || journal.facts.identity == summary.facts.identity
            || payload.as_ref().is_some_and(|p| {
                !matches!(p.object, DiagnosticObject::Payload(actual, _, sugarsubstitute_contracts::incident::AttachmentKind::Minidump) if actual == key)
                    || p.id == journal.id || p.id == summary.id
                    || p.facts.identity == journal.facts.identity || p.facts.identity == summary.facts.identity
            }) {
            return Err(error(operation.key, Code::InvalidIdentity));
        }
        let id = next_lease(&mut state, operation.key)?;
        state.data_mut()?.capture = Some(RetainedCapture {
            id,
            journal,
            summary,
            payload,
            callback_active: false,
            native_settled: false,
            delete_attempted: false,
            delete_issued: false,
            journal_closed: false,
        });
        Ok(DiagnosticCaptureIntent { shared: self.shared.clone(), operation: operation.key, id })
    }
}
fn pin(
    state: &State,
    shared: &Shared,
    read: &DiagnosticReadLease,
    key: LogicalStoreOperationKey,
) -> Result<CapturePin, DiagnosticStorageError> {
    if !Shared::ptr_eq(shared, &read.shared) || read.operation != key {
        return Err(error(key, Code::InvalidIdentity));
    }
    let actual =
        state.reads.iter().find(|r| r.id == read.id).ok_or_else(|| error(key, Code::Closed))?;
    if actual.facts != Some(read.facts) {
        return Err(error(key, Code::ChangedIdentity));
    }
    Ok(CapturePin {
        id: read.id,
        native: actual.native.filter(|id| *id != 0).ok_or_else(|| error(key, Code::Closed))?,
        object: actual.object.ok_or_else(|| error(key, Code::InvalidIdentity))?,
        facts: read.facts,
    })
}
