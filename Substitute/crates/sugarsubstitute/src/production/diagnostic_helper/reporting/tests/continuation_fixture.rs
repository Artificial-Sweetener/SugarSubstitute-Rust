// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real protected CAS with the existing external filesystem boundary fixture only.
use super::*;

/// Retain a genuine presented CAS and its file custody for independent checked cleanup.
/// This creates no received Reporter frame or native action authority.
pub(in crate::production) fn presented_fixture(
    context: &JobContext,
) -> Result<(ReportingOperation, DiagnosticStorageFixture, IncidentSummary), TestError> {
    let fixture = DiagnosticStorageFixture::new()?;
    let mut operation = prepared(context, &fixture)?;
    operation.record_attempt(binding(&original()?)?, ReportAttempt::Presented, context)?;
    let summary = operation.attempt().ok_or("Actual presented CAS missing")?.summary().clone();
    Ok((operation, fixture, summary))
}
/// Execute the queued command and its exact logical operation against retained canonical bytes.
/// The caller obtains no Reporter capability or native ACK authority from this storage fixture.
pub(in crate::production) fn acknowledge_fixture(
    fixture: DiagnosticStorageFixture,
    logical: LogicalStoreOperationKey,
    command: ReportCommand,
    at: IncidentUtc,
    context: &JobContext,
) -> Result<(ReportingOperation, IncidentSummary), TestError> {
    let mut storage = fixture.storage(context)?;
    let operation = storage.begin_operation(logical, context)?;
    let opening = fixture.resolve_opening(&mut storage, &operation, context)?;
    storage.acquire_writer(&operation, context)?;
    let mut repository = IncidentRepository::new(storage, operation, context)?;
    repository.validate_store(context)?;
    let mut operation = ReportingOperation::new();
    operation.repository = Some(repository);
    operation.opening = Some(opening);
    operation.acknowledge(command, at, context)?;
    let proof = operation.acknowledgement().ok_or("Actual acknowledgement CAS missing")?;
    let summary = proof.committed().summary().clone();
    assert_eq!(proof.committed().operation(), logical);
    assert_eq!(proof.acknowledgement().at(), at);
    assert_eq!(proof.committed().payload_revision(), Some(original()?.revision()));
    Ok((operation, summary))
}

impl ReportingOperation {
    /// Run real checked retirement and return its exact result with all operation custody retained.
    pub(in crate::production) fn retire_fixture(mut self, context: &JobContext) -> (bool, Self) {
        let retired = self.retire(context);
        (retired, self)
    }
    /// Borrow the actual cleanup failure retained by the authoritative repository owner.
    pub(in crate::production) fn fixture_retirement_cause(&self) -> Option<&IncidentStoreError> {
        self.retirement.as_ref()
    }
}
