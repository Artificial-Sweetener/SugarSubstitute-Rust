// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Interleaved view preparation crosses the actual central admission boundary before export IDs.
use super::*;
use crate::completion_wake::Wake;
use std::{num::NonZeroUsize, sync::Arc};
use sugarsubstitute_execution::{ExecutionOwner, ReportExportPairRequest};

#[test]
fn export_ids_follow_real_view_jobs_and_both_cleanup_roles_and_pair_reach_actual_terminal()
-> Result<()> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let sequence = crate::job_sequence::JobSequence::new();
    let scopes = crate::scope_sequence::ScopeSequence::new();
    let view_scope =
        client.create_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, None)?;
    let until = Moment::from_nanos(60_000_000_017);
    let mut export = ExportJobs::new(&client, &scopes, until)?;
    assert!(export.source_spec().is_err());
    let mut last = None;
    // Native repository effects are outside this case: real admitted preparation
    // jobs own their original view/counter storage and advance the same global IDs.
    for bytes in [131_072, 65_536, 16_384] {
        let specification = JobSpec {
            id: sequence.allocate()?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(until),
            reservation_bytes: bytes,
            replacement: None,
        };
        let mut job = client.submit_blocking(&view_scope, specification, move |context| {
            context.memory().claim(bytes)
        })?;
        let completed = loop {
            let _actual = batch(&client, &wake)?;
            if let Some(done) = job.try_take_completion() {
                break done;
            }
        };
        assert!(matches!(completed.outcome, JobOutcome::Success(_)));
        assert_eq!(completed.spec.deadline, Some(until));
        last = Some(specification.id);
        drop(completed);
    }
    export.prepare(&sequence)?;
    assert!(Some(export.source_cleanup_spec()?.id) > last);
    let source_spec = export.source_spec()?;
    let receiving_spec = export.receiving_spec()?;
    let source_cleanup = client
        .reserve_symbol_cleanup(
            &export.source_cleanup,
            export.source_cleanup_spec()?,
            &export.source,
            source_spec,
            |_| Ok::<_, u8>(true),
        )
        .map_err(|rejection| rejection.error)?;
    let receiving_cleanup = client
        .reserve_report_export_cleanup(
            &export.receiving_cleanup,
            export.receiving_cleanup_spec()?,
            &export.receiving,
            receiving_spec,
            |_| Ok::<_, u8>(true),
        )
        .map_err(|rejection| rejection.error)?;
    let (mut source, mut receiving) = client
        .submit_report_export_pair(
            ReportExportPairRequest::new(
                &export.source,
                source_spec,
                &source_cleanup,
                &export.receiving,
                receiving_spec,
                &receiving_cleanup,
            ),
            move |source, receiving| {
                assert_eq!(source.deadline(), Some(until));
                assert_eq!(receiving.deadline(), Some(until));
                Ok::<_, u8>(true)
            },
        )
        .map_err(|rejection| rejection.error)?;
    let mut done_source = None;
    let mut done_receiving = None;
    while done_source.is_none() || done_receiving.is_none() {
        let _actual = batch(&client, &wake)?;
        if done_source.is_none() {
            done_source = source.try_take_completion();
        }
        if done_receiving.is_none() {
            done_receiving = receiving.try_take_completion();
        }
    }
    assert!(matches!(
        done_source.as_ref().map(|done| &done.outcome),
        Some(JobOutcome::Success(true))
    ));
    assert!(matches!(
        done_receiving.as_ref().map(|done| &done.outcome),
        Some(JobOutcome::Success(true))
    ));
    let source_cutoff =
        client.freeze_symbol_cleanup_deadline(&source_cleanup, Duration::from_secs(15))?;
    let receiving_cutoff = client
        .freeze_report_export_cleanup_deadline(&receiving_cleanup, Duration::from_secs(15))?;
    let mut source_cleanup =
        source_cleanup.begin(source_cutoff).map_err(|rejection| rejection.error)?;
    let mut receiving_cleanup =
        receiving_cleanup.begin(receiving_cutoff).map_err(|rejection| rejection.error)?;
    let mut done_source_cleanup = None;
    let mut done_receiving_cleanup = None;
    while done_source_cleanup.is_none() || done_receiving_cleanup.is_none() {
        let _actual = batch(&client, &wake)?;
        if done_source_cleanup.is_none() {
            done_source_cleanup = source_cleanup.try_take_completion();
        }
        if done_receiving_cleanup.is_none() {
            done_receiving_cleanup = receiving_cleanup.try_take_completion();
        }
    }
    assert!(matches!(
        done_source_cleanup.as_ref().map(|done| &done.outcome),
        Some(JobOutcome::Success(true))
    ));
    assert!(matches!(
        done_receiving_cleanup.as_ref().map(|done| &done.outcome),
        Some(JobOutcome::Success(true))
    ));
    assert_eq!(export.until(), until);
    drop(owner);
    drop((done_source, done_receiving, done_source_cleanup, done_receiving_cleanup));
    assert_eq!(export.source.snapshot().reserved_bytes, 0);
    assert_eq!(export.receiving.snapshot().reserved_bytes, 0);
    assert_eq!(export.source_cleanup.snapshot().reserved_bytes, 0);
    assert_eq!(export.receiving_cleanup.snapshot().reserved_bytes, 0);
    Ok(())
}

/// Fixed original clock: this test does not wait for time or create a deadline successor.
struct TestClock;
impl Clock for TestClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(17)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
/// Drain original notices, sleeping only on the owner's causal wake and never on elapsed time.
fn batch(client: &ExecutionClient, wake: &Wake) -> Result<CompletionBatch> {
    wake.until(|| match client.drain_completions(NonZeroUsize::MIN.saturating_add(31)) {
        Ok(actual) if actual.notifications.is_empty() => None,
        other => Some(other.map_err(Into::into)),
    })
}
