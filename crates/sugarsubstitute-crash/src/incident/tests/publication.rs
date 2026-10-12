// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real repository commits and custody through the protected external fixture.
use super::*;
use std::{
    num::NonZeroUsize,
    path::Path,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::ExecutionOwner;
use sugarsubstitute_supervision::diagnostic_storage::{fixture::*, *};
type TestError = Box<dyn Error + Send + Sync>;
type TestResult = Result<(), TestError>;
struct TestClock;
impl Clock for TestClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(17)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
struct Wake {
    count: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *self.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
pub(super) fn admitted(work: impl FnOnce(JobContext) -> TestResult + Send + 'static) -> TestResult {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(100)),
            reservation_bytes: DIAGNOSTIC_STORAGE_BYTES,
            replacement: None,
        },
        work,
    )?;
    let completion = loop {
        let old = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(completion) = job.try_take_completion() {
            break completion;
        }
        let (count, timeout) = wake
            .changed
            .wait_timeout_while(
                wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
                Duration::from_secs(10),
                |count| *count == old,
            )
            .map_err(|cause| std::io::Error::other(cause.to_string()))?;
        if timeout.timed_out() && *count == old {
            return Err("Repository completion stalled".into());
        }
    };
    match completion.outcome {
        JobOutcome::Success(()) => (),
        JobOutcome::Failure(cause) => {
            let mut chain = Vec::new();
            let mut current: Option<&dyn Error> = Some(cause.as_ref());
            while let Some(error) = current {
                chain.push(error.to_string());
                current = error.source();
            }
            return Err(chain.join(" -> ").into());
        }
        outcome => return Err(format!("Repository worker outcome: {outcome:?}").into()),
    }
    drop(completion.memory);
    drop(job);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
pub(super) fn setup(
    context: &JobContext,
) -> Result<(DiagnosticStorageFixture, IncidentRepository), TestError> {
    let fixture = DiagnosticStorageFixture::new()?;
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let key = LogicalStoreOperationKey::new(owner, IncidentOperationId::new(1)?);
    let ledger = SummaryEvictions::new(owner.namespace, 0, 0, 0, None)?;
    fixture.put(
        DiagnosticObject::StoreState,
        &encode_store_state(&IncidentStoreState::Ledger(ledger))?,
    )?;
    let mut storage = fixture.storage(context)?;
    let operation = storage.begin_operation(key, context)?;
    storage.resolve(
        Path::new("external-repository-fixture"),
        owner.namespace,
        &operation,
        context,
    )?;
    storage.acquire_writer(&operation, context)?;
    storage.inventory(&operation, context)?;
    Ok((fixture, IncidentRepository::new(storage, operation)))
}
fn test_seed() -> Result<IncidentSeed, TestError> {
    super::seed().map_err(|cause| std::io::Error::other(cause.to_string()).into())
}
fn test_summary() -> Result<IncidentSummary, TestError> {
    super::summary().map_err(|cause| std::io::Error::other(cause.to_string()).into())
}
#[test]
fn repository_seed_actual_repeat_and_changed_content_preserves_original() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let seed = test_seed()?;
        let first = repository.publish_seed(seed.clone(), &context)?;
        let changes = fixture.snapshot().mutation_count();
        let repeat = repository.publish_seed(seed.clone(), &context)?;
        assert_eq!(first.content_sha256(), repeat.content_sha256());
        assert_eq!(fixture.snapshot().mutation_count(), changes);
        let changed = IncidentSeed::new(
            seed.key(),
            seed.revision(),
            seed.build().clone(),
            seed.created(),
            IncidentStage::Running,
        )?;
        assert!(matches!(
            repository.publish_seed(changed, &context),
            Err(IncidentStoreError::ChangedDuplicate)
        ));
        assert_eq!(fixture.snapshot().mutation_count(), changes);
        assert_eq!(repository.snapshot().reads(), 1);
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok(())
    })
}
#[test]
fn repository_summary_commit_load_and_stale_cas_are_actual() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let summary = test_summary()?;
        let mut ticket = repository.begin_publish(
            summary.seed().clone(),
            ExpectedSummary::Absent,
            IncidentOperationId::new(1)?,
            &context,
        )?;
        let committed = repository.commit(&mut ticket, summary.clone(), &context)?;
        assert_eq!(committed.summary(), &summary);
        assert_eq!(
            committed.content_sha256(),
            super::super::codec_ledger::digest(&encode_summary(&StoredIncidentSummary::new(
                summary.clone(),
                None
            )?)?)
        );
        let loaded = repository.load(committed.key(), &context)?;
        assert_eq!(loaded.summary(), &summary);
        assert_eq!(loaded.evictions().pending(), 0);
        let before = fixture.snapshot().mutation_count();
        assert!(matches!(
            repository.begin_publish(
                summary.seed().clone(),
                ExpectedSummary::Absent,
                IncidentOperationId::new(1)?,
                &context
            ),
            Err(IncidentStoreError::StaleRevision)
        ));
        assert_eq!(fixture.snapshot().mutation_count(), before);
        let mut next = repository.begin_publish(
            summary.seed().clone(),
            ExpectedSummary::Exact {
                revision: committed.revision(),
                content_sha256: committed.content_sha256(),
            },
            IncidentOperationId::new(1)?,
            &context,
        )?;
        let updated = IncidentSummary::new(
            summary.seed().clone(),
            summary.revision().next()?,
            summary.facts().clone(),
        )?;
        assert_eq!(repository.commit(&mut next, updated, &context)?.revision().get(), 3);
        repository.retire(&context)?;
        Ok(())
    })
}
#[test]
fn repository_failed_activation_keeps_stage_and_physical_charge() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        fixture.set_failure(DiagnosticFixtureFailure::PostActivation, true);
        assert!(matches!(
            repository.publish_seed(test_seed()?, &context),
            Err(IncidentStoreError::Storage(_))
        ));
        assert_eq!(repository.snapshot().stages(), 1);
        assert_eq!(fixture.snapshot().live_outputs(), 1);
        assert!(context.memory().claim(1).is_err());
        fixture.set_failure(DiagnosticFixtureFailure::Close, true);
        assert!(repository.retire(&context).is_err());
        assert_eq!(fixture.snapshot().live_outputs(), 1);
        fixture.set_failure(DiagnosticFixtureFailure::Close, false);
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok(())
    })
}
#[test]
fn repository_ticket_drop_keeps_admission_and_foreign_operation_cannot_publish() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        assert!(
            repository
                .begin_publish(
                    test_seed()?,
                    ExpectedSummary::Absent,
                    IncidentOperationId::new(2)?,
                    &context
                )
                .is_err()
        );
        assert_eq!(fixture.snapshot().mutation_count(), 0);
        {
            let _ticket = repository.begin_publish(
                test_seed()?,
                ExpectedSummary::Absent,
                IncidentOperationId::new(1)?,
                &context,
            )?;
        }
        assert!(
            repository
                .begin_publish(
                    test_seed()?,
                    ExpectedSummary::Absent,
                    IncidentOperationId::new(1)?,
                    &context
                )
                .is_err()
        );
        repository.retire(&context)?;
        Ok(())
    })
}
#[test]
fn repository_text_payload_requires_actual_digest_and_changed_source_blocks_load() -> TestResult {
    admitted(|context| {
        let (fixture, mut repository) = setup(&context)?;
        let original = test_summary()?;
        let mut ticket = repository.begin_publish(
            original.seed().clone(),
            ExpectedSummary::Absent,
            IncidentOperationId::new(1)?,
            &context,
        )?;
        let chunk = RedactedTextChunk::from_facts(&[RedactedDiagnosticFact::Failure(
            FailureEvidence::Unexplained,
        )])?;
        let bytes = chunk.as_bytes().to_vec();
        let actual_digest = super::super::codec_ledger::digest(&bytes);
        let original_bytes = chunk.original_bytes();
        let omitted = chunk.omitted_bytes();
        repository.append_text(&mut ticket, AttachmentKind::Fault, chunk, &context)?;
        let summary = |hash| -> Result<IncidentSummary, IncidentStoreError> {
            let facts = original.facts();
            Ok(IncidentSummary::new(
                original.seed().clone(),
                original.revision(),
                SummaryFacts::new(
                    facts.retention(),
                    facts.exit(),
                    facts.failure(),
                    facts.capture(),
                    PayloadDisposition::Retained(AttachmentInventory::new(&[
                        AttachmentFacts::new(
                            AttachmentKind::Fault,
                            bytes.len() as u64,
                            original_bytes,
                            omitted,
                            hash,
                        )?,
                    ])?),
                    facts.reporting(),
                    facts.symbols().clone(),
                ),
            )?)
        };
        let before = fixture.snapshot().mutation_count();
        assert!(matches!(
            repository.commit(&mut ticket, summary([9; 32])?, &context),
            Err(IncidentStoreError::WrongDigest)
        ));
        assert_eq!(fixture.snapshot().mutation_count(), before);
        let committed = repository.commit(&mut ticket, summary(actual_digest)?, &context)?;
        assert_eq!(committed.payload_revision(), Some(committed.revision()));
        assert_eq!(repository.load(committed.key(), &context)?.summary(), committed.summary());
        let facts = committed.summary().facts();
        let advanced = IncidentSummary::new(
            committed.summary().seed().clone(),
            committed.revision().next()?,
            SummaryFacts::new(
                facts.retention(),
                facts.exit(),
                facts.failure(),
                facts.capture(),
                facts.payload().clone(),
                ReportDisposition::new(1, Some(ReportAttempt::Presented))?,
                facts.symbols().clone(),
            ),
        )?;
        let stored = StoredIncidentSummary::new(advanced.clone(), committed.payload_revision())?;
        fixture.put(DiagnosticObject::Summary(committed.key()), &encode_summary(&stored)?)?;
        let before = fixture.snapshot().mutation_count();
        let loaded = repository.load(committed.key(), &context)?;
        assert_eq!(loaded.summary(), &advanced);
        assert_eq!(loaded.payload_revision(), Some(committed.revision()));
        assert_eq!(fixture.snapshot().mutation_count(), before);
        fixture.put(
            DiagnosticObject::Payload(committed.key(), committed.revision(), AttachmentKind::Fault),
            &vec![b'x'; bytes.len()],
        )?;
        assert!(matches!(
            repository.load(committed.key(), &context),
            Err(IncidentStoreError::WrongDigest)
        ));
        assert_eq!(repository.snapshot().reads(), 1);
        repository.retire(&context)?;
        assert_eq!(fixture.snapshot().live_outputs(), 0);
        Ok(())
    })
}
