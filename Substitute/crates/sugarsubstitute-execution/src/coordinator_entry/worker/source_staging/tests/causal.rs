// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve the last-provider and actual charged source-transfer fences.
use super::*;

#[test]
fn last_original_provider_closes_actual_client_ancestor_before_launch_staging() -> TestResult {
    let mut fixture = Fixture::new()?;
    let clock = fixture.clock.clone();
    let core = fixture.core.clone();
    let parent = fixture.parent.id();
    fixture.clock.on_next_current_thread_observation(Box::new(move || {
        clock.on_next_current_thread_observation(Box::new(move || core.close_scope(parent)));
    }));
    let error =
        fixture.active.allocate(&fixture.core).err().ok_or("Closed ancestor staged Launch")?;
    assert!(matches!(
        error
            .native_cause()
            .and_then(std::error::Error::source)
            .and_then(|cause| cause.downcast_ref::<EntryPreparationInputError>()),
        Some(EntryPreparationInputError::Closed)
    ));
    assert!(!fixture.active.context.cancellation_requested());
    assert!(!fixture.client_scope.snapshot().closed);
    assert!(fixture.active.client.is_none());
    assert!(fixture.active.source_pending.is_none());
    assert!(fixture.active.work.input.is_some());
    assert!(
        fixture.active.connections[0]
            .write
            .as_ref()
            .ok_or("Missing charged write")?
            .bytes
            .iter()
            .all(|byte| *byte == 0)
    );
    assert_eq!(fixture.original.snapshot().reserved_bytes, 4 * 1024 * 1024);
    Ok(())
}

#[test]
fn current_client_validation_ignores_substituted_ports_but_observes_actual_record_cancellation()
-> TestResult {
    let mut fixture = Fixture::new()?;
    fixture.retain_source();
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = &fixture.active.context;
    fixture.active.context = JobContext::new(
        observed.owner_id(),
        observed.scope_id(),
        observed.spec(),
        observed.deadline(),
        Arc::new(ForeignPorts(calls.clone())),
        Arc::new(ForeignPorts(calls.clone())),
        AdmittedJobResources::new(observed.memory().clone(), None),
    );
    fixture.active.source_fence(&fixture.core, true)?;
    fixture.core.cancel_job(fixture.active.context.job_id());
    assert!(fixture.active.source_fence(&fixture.core, true).is_err());
    assert_eq!(calls.load(Ordering::Acquire), 0);
    Ok(())
}

#[test]
fn failed_launch_remains_primary_when_original_post_control_is_cancelled() -> TestResult {
    let mut fixture = Fixture::new()?;
    fixture.retain_source();
    fixture.active.source_pending =
        Some(EntrySourceOutcome::Launch(Err(EntryTransportError::Malformed)));
    let core = fixture.core.clone();
    let original = fixture.original.id();
    fixture.clock.on_next_current_thread_observation(Box::new(move || core.cancel_scope(original)));
    let error = fixture
        .active
        .complete_source_step(&fixture.core)
        .err()
        .ok_or("Primary failure disappeared")?;
    assert!(
        matches!(error,EntryTransportError::PreparedRetirement(ref retained) if retained.original().is_some_and(|cause|cause.kind()==crate::EntryPreparedCauseKind::Malformed))
    );
    assert!(fixture.active.source_pending.is_none());
    assert!(fixture.active.work.input.is_some());
    assert_eq!(fixture.original.snapshot().reserved_bytes, 4 * 1024 * 1024);
    Ok(())
}

#[test]
fn failed_destination_clock_still_closes_and_retains_original_post_observation() -> TestResult {
    let mut fixture = Fixture::new()?;
    fixture.retain_source();
    fixture.clock.on_next_current_thread_observation(Box::new(|| {
        std::panic::resume_unwind(Box::new("destination provider failure"))
    }));
    let error = fixture
        .active
        .source_fence(&fixture.core, true)
        .err()
        .ok_or("Fatal destination continued")?;
    assert!(
        matches!(error,EntryTransportError::PreparedRetirement(ref retained) if retained.original().is_some_and(|cause|cause.kind()==crate::EntryPreparedCauseKind::Clock))
    );
    assert!(
        fixture
            .prepared
            .owner
            .original_binding()?
            .check_publication(&fixture.core, fixture.prepared.owner.original_binding()?.setup())
            .is_err()
    );
    assert_eq!(fixture.original.snapshot().reserved_bytes, 4 * 1024 * 1024);
    Ok(())
}

#[test]
fn actual_charged_transfer_drops_complete_source_before_read_buffer_admission() -> TestResult {
    let mut fixture = Fixture::new()?;
    fixture.active.allocate(&fixture.core)?;
    assert!(fixture.active.work.input.is_none());
    assert!(fixture.active.source_owner.is_none());
    assert!(fixture.active.source_input_claim.is_none());
    assert!(fixture.active.connections[0].read.is_some());
    assert!(fixture.active.client.as_ref().is_some_and(|client| client.length > 0));
    assert!(fixture.client_scope.snapshot().reserved_bytes <= MIN_ENTRY_CLIENT_BYTES);
    assert_eq!(fixture.original.snapshot().reserved_bytes, 4 * 1024 * 1024);
    Ok(())
}
