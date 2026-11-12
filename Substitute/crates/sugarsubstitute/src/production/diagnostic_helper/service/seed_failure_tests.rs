// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual source loans, finite completion capture and checked cleanup with external storage fakes.
//! The inert service fixture supplies no namespace capability and qualifies no native path.
use super::super::{
    SeedOperation,
    tests::{TestClock, observe, prepared, seed, spec},
};
use super::*;
use crate::completion_wake::Wake;
use std::{
    io::{self, Write},
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};
use sugarsubstitute_execution::ExecutionOwner;
use sugarsubstitute_supervision::diagnostic_storage::fixture::{
    DiagnosticFixtureFailure, DiagnosticStorageFixture,
};
type TestError = Box<dyn Error + Send + Sync>;

#[derive(Clone, Default)]
struct Output(Arc<Mutex<Vec<u8>>>);
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Output {
    fn observe(&self) -> tracing::subscriber::DefaultGuard {
        let output = self.clone();
        tracing::subscriber::set_default(
            tracing_subscriber::fmt()
                .without_time()
                .with_ansi(false)
                .with_writer(move || output.clone())
                .finish(),
        )
    }
    fn text(&self) -> Result<String, std::string::FromUtf8Error> {
        String::from_utf8(self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone())
    }
}

fn delivered(
    service: &mut SeedService,
    client: &ExecutionClient,
    wake: &Wake,
    cleanup: bool,
) -> Result<(), TestError> {
    loop {
        let before = wake.revision();
        let batch = client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        if cleanup {
            service.capture_cleanup(&batch)?;
        } else {
            service.capture(&batch)?;
        }
        if (cleanup && service.cleanup_completion.is_some())
            || (!cleanup && service.completion.is_some())
        {
            return Ok(());
        }
        if batch.remaining > 0 {
            continue;
        }
        wake.wait_after(before);
    }
}

fn exercise(
    operation: impl FnOnce(&mut SeedOperation, &JobContext) -> Result<(), TestError> + Send + 'static,
    inspect: impl FnOnce(&SeedService, &str) -> Result<(), TestError>,
) -> Result<(), TestError> {
    let wake = Arc::new(Wake::default());
    let execution = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = execution.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let jobs = JobSequence::new();
    let mut construction = spec(jobs.allocate()?);
    construction.reservation_bytes = crate::production::incident_host::CONTROLLER_BYTES;
    let mut constructor =
        client.submit_blocking(&scope, construction, |context| SeedService::new(&context))?;
    let completion = observe(&client, &wake, &mut constructor)?;
    let mut service = match completion.outcome {
        JobOutcome::Success(service) => service,
        _ => return Err("Actual seed source construction did not succeed".into()),
    };
    drop(completion.memory);
    drop(constructor);
    let source = service.source.alias();
    let original = spec(jobs.allocate()?);
    service.job = Some(client.submit_blocking(&scope, original, move |context| {
        let mut loan = source.loan()?;
        operation(loan.operation()?, &context).map_err(|_| SeedServiceIssue::Identity)?;
        Ok::<_, SeedServiceIssue>(())
    })?);
    // Only external operation setup is supplied here; actual worker, delivery, logical
    // failure adoption, checked retirement and all original claims use production owners.
    service.installed = true;
    service.requested =
        Some((scope.owner_id(), scope.id(), original.deadline.ok_or("No original cutoff")?));
    delivered(&mut service, &client, &wake, false)?;
    assert_eq!(service.outcome(), Some(TerminalKind::Success));
    assert!(!service.retain_failure());
    service.submit_cleanup(&client, &scope, &jobs, Moment::from_nanos(100))?;
    delivered(&mut service, &client, &wake, true)?;
    assert!(service.files_retired());
    let output = Output::default();
    let _subscriber = output.observe();
    assert!(service.retain_failure());
    let original_error = std::ptr::from_ref(service.failure().ok_or("Original failure lost")?);
    assert!(service.retain_failure());
    assert!(std::ptr::eq(
        original_error,
        std::ptr::from_ref(service.failure().ok_or("Failure replaced")?)
    ));
    assert_eq!(service.outcome(), Some(TerminalKind::Success));
    assert!(service.files_retired());
    assert_eq!(service.release_adopted(), Err(SeedServiceIssue::Busy));
    let text = output.text()?;
    assert_eq!(text.matches("incident_seed_transaction_failed").count(), 1);
    assert!(text.contains("worker_terminal=Success"));
    inspect(&service, &text)?;
    let retained =
        crate::production::incident_host::CONTROLLER_BYTES + DIAGNOSTIC_STORAGE_BYTES + 32 * 1024;
    drop(execution);
    assert_eq!(scope.snapshot().reserved_bytes, retained);
    drop(service);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn successful_seed_worker_retains_actual_publication_failure_after_checked_cleanup()
-> Result<(), TestError> {
    exercise(
        |operation, context| {
            let fixture = DiagnosticStorageFixture::new()?;
            *operation = prepared(context, &fixture)?;
            fixture.set_failure(DiagnosticFixtureFailure::PartialCreate, true);
            operation.first = Some(
                operation
                    .publish(seed()?, context)
                    .err()
                    .ok_or("Partial publication unexpectedly succeeded")?,
            );
            assert!(operation.failure().and_then(Error::source).is_some());
            assert!(operation.committed().is_none());
            Ok(())
        },
        |service, text| {
            let original = service
                .failure()
                .and_then(Error::source)
                .and_then(|cause| cause.downcast_ref::<super::super::IncidentStoreError>())
                .ok_or("Original storage cause lost")?;
            assert!(original.source().is_some());
            assert!(text.contains("stage=Initialize"));
            assert!(text.contains("store_code=Some(Storage)"));
            assert!(!text.contains(&original.to_string()));
            assert_eq!(service.inspect(|operation| operation.committed().is_none()), Some(true));
            Ok(())
        },
    )
}

#[test]
fn successful_seed_worker_retains_actual_memory_failure_and_original_byte_counts()
-> Result<(), TestError> {
    exercise(
        |operation, context| {
            let _held = context.memory().claim(DIAGNOSTIC_STORAGE_BYTES)?;
            assert!(operation.claim_memory(context).is_none());
            Ok(())
        },
        |service, text| {
            let original = service
                .failure()
                .and_then(Error::source)
                .and_then(|cause| cause.downcast_ref::<MemoryClaimError>())
                .ok_or("Original memory cause lost")?;
            assert_eq!(original.requested_bytes, DIAGNOSTIC_STORAGE_BYTES);
            assert_eq!(original.available_bytes, 0);
            assert!(text.contains("stage=Memory"));
            assert!(text.contains(&format!("memory_requested_bytes={DIAGNOSTIC_STORAGE_BYTES}")));
            assert!(text.contains("memory_available_bytes=0"));
            Ok(())
        },
    )
}

#[test]
fn seed_failure_diagnostic_redacts_external_codec_text_and_preserves_original_cause()
-> Result<(), TestError> {
    exercise(
        |operation, _| {
            operation.first = Some(super::super::IncidentStoreError::Codec(
                <serde_json::Error as serde::de::Error>::custom("private-seed-path-and-token"),
            ));
            Ok(())
        },
        |service, text| {
            let original = service
                .failure()
                .and_then(Error::source)
                .and_then(Error::source)
                .ok_or("Original codec cause lost")?;
            assert!(original.to_string().contains("private-seed-path-and-token"));
            assert!(text.contains("store_code=Some(Codec)"));
            assert!(!text.contains("private-seed-path-and-token"));
            assert!(!format!("{:?}", service.failure()).contains("private-seed-path-and-token"));
            Ok(())
        },
    )
}
