// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise physical publication and journal I/O while the actual finished roles remain held.
//! Semantic incident codecs and native capture selection retain their separate owner tests.
use super::*;
use std::io::Write;
use sugarsubstitute_contracts::lifecycle::{InstanceId, RunGeneration, RunKey};

/// Retain only the actual closed allocation category and live ledger in a fixture failure.
fn failed(
    storage: &DiagnosticStorage,
    phase: &'static str,
    cause: DiagnosticStorageError,
) -> TestError {
    let native = Error::source(&cause).and_then(|value| value.downcast_ref::<crate::ClaimError>());
    let inner = native
        .and_then(Error::source)
        .and_then(|value| value.downcast_ref::<DiagnosticNativeAllocationFailure>());
    format!("finished physical publication {phase}: outer={:?} inner={:?} post={:?} residual={:?} limit={}",
        cause.code(), inner.map(DiagnosticNativeAllocationFailure::code),
        inner.and_then(DiagnosticNativeAllocationFailure::post_control_failure),
        storage.shared._allocation.fixture_residual_bytes(), DIAGNOSTIC_RESIDUAL_BYTES).into()
}
/// Write, flush and activate an actual protected record with the original fixed buffer.
fn publish_record(
    storage: &mut DiagnosticStorage,
    operation: &DiagnosticStorageOperation,
    context: &JobContext,
    object: DiagnosticObject,
    image: &[u8],
) -> Result<DiagnosticActivation, TestError> {
    let mut bytes = storage.record_buffer(operation, context)?;
    let decode = storage.recovery_decode(operation, context)?;
    bytes.write_all(image)?;
    drop(decode);
    drop(
        storage
            .inventory(operation, context)
            .map_err(|cause| failed(storage, "record-inventory", cause))?,
    );
    let mut stage = storage
        .stage(object, bytes.as_bytes().len() as u64, operation, context)
        .map_err(|cause| failed(storage, "record-stage", cause))?;
    stage.write_chunk(bytes.as_bytes(), context)?;
    let flushed = stage.flush(context)?.read_facts();
    assert_eq!(flushed.sha256(), <[u8; 32]>::from(Sha256::digest(image)));
    let activation = storage
        .activate(&mut stage, DiagnosticPredecessor::Absent, operation, context)
        .map_err(|cause| failed(storage, "record-activation", cause))?;
    assert_eq!(activation.facts(), flushed);
    drop(stage);
    Ok(activation)
}
/// Hold publication destinations through physical payload, Summary and Journal ownership.
pub(super) fn roundtrip(
    storage: &mut DiagnosticStorage,
    operation: &DiagnosticStorageOperation,
    context: &JobContext,
) -> TestResult {
    let incident = IncidentKey::new(IncidentSubject::Desktop(RunKey {
        owner: operation.key().owner(),
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    }));
    let revision = IncidentRevision::new(1)?;
    let seed_object = DiagnosticObject::Seed(incident);
    // These are actual bounded protected bytes; no semantic Summary or native proof is minted.
    let seed = publish_record(storage, operation, context, seed_object, b"physical-seed")?;
    let preflight = storage.recovery_preflight(operation, context)?;
    let identity = storage.recovery_publication_identity(operation, context)?;
    let token = identity.retain_for_token(operation, context)?;
    let pending = storage.recovery_pending(operation, context)?;
    let inventory = storage.inventory(operation, context)?;
    let candidate = inventory
        .entries()
        .find(|entry| entry.class() == DiagnosticEntryClass::Seed)
        .ok_or("Published physical seed absent")?;
    let mut seed_read = storage
        .read_inventory(candidate, 4096, operation, context)
        .map_err(|cause| failed(storage, "preflight-seed-read", cause))?;
    let seed_bytes = seed_read.whole_source(context)?;
    assert_eq!(seed_bytes.as_bytes(), b"physical-seed");
    assert_eq!(seed_read.facts(), seed.facts());
    drop(seed_bytes);
    seed_read.retire(context)?;
    drop(seed_read);
    drop(inventory);

    let payload_object = DiagnosticObject::Payload(incident, revision, AttachmentKind::Minidump);
    let chunk = [0x5a_u8; 4096];
    let payload_bytes = chunk.len() as u64 * 20;
    drop(storage.inventory(operation, context)?);
    let mut stage = storage
        .stage(payload_object, payload_bytes, operation, context)
        .map_err(|cause| failed(storage, "payload-stage", cause))?;
    for _ in 0..20 {
        stage.write_chunk(&chunk, context)?;
    }
    let flushed = stage.flush(context)?.read_facts();
    assert_eq!(flushed.bytes(), payload_bytes);
    let payload = storage
        .activate(&mut stage, DiagnosticPredecessor::Absent, operation, context)
        .map_err(|cause| failed(storage, "payload-activation", cause))?;
    assert_eq!(payload.facts(), flushed);
    drop(stage);

    let summary_object = DiagnosticObject::Summary(incident);
    let summary = publish_record(storage, operation, context, summary_object, b"physical-summary")?;
    let record = storage.recovery_record(operation, context)?;
    let mut summary_read = storage
        .read(summary_object, DIAGNOSTIC_IO_BYTES as u64, operation, context)
        .map_err(|cause| failed(storage, "committed-summary-read", cause))?;
    let mut payload_read = storage
        .read(payload_object, payload_bytes, operation, context)
        .map_err(|cause| failed(storage, "committed-payload-read", cause))?;
    assert_eq!(summary_read.facts(), summary.facts());
    assert_eq!(payload_read.facts(), payload.facts());
    let journal_object = DiagnosticObject::Journal(operation.key());
    let journal = publish_record(storage, operation, context, journal_object, b"physical-journal")?;
    let mut journal_read = storage
        .read(journal_object, DIAGNOSTIC_IO_BYTES as u64, operation, context)
        .map_err(|cause| failed(storage, "committed-journal-read", cause))?;
    assert_eq!(journal_read.facts(), journal.facts());
    // The capture read plus these three exact committed reads fill the existing four-pin bound.
    assert_eq!(storage.snapshot().reads(), DIAGNOSTIC_READ_BOUND);
    let inventory = storage
        .inventory(operation, context)
        .map_err(|cause| failed(storage, "committed-journal-inventory", cause))?;
    assert_eq!(inventory.status(), DiagnosticInventoryStatus::Complete);
    let summary_bytes = summary_read.whole_source(context)?;
    let journal_bytes = journal_read.whole_source(context)?;
    assert_eq!(summary_bytes.as_bytes(), b"physical-summary");
    assert_eq!(journal_bytes.as_bytes(), b"physical-journal");
    let mut actual = [0; 4096];
    for offset in 0..20 {
        assert_eq!(
            payload_read.read_chunk(offset * actual.len() as u64, &mut actual, context)?,
            actual.len()
        );
        assert_eq!(actual, chunk);
    }
    drop(summary_bytes);
    drop(journal_bytes);
    summary_read.retire(context)?;
    payload_read.retire(context)?;
    journal_read.retire(context)?;
    drop(summary_read);
    drop(payload_read);
    drop(journal_read);
    drop(inventory);
    drop(record);
    drop(pending);
    drop(token);
    drop(identity);
    drop(preflight);
    Ok(())
}
