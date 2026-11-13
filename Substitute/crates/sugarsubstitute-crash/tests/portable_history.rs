// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the public application-adapter contract without paths or native mutation authority.
#![cfg(windows)]
use std::{
    error::Error,
    io,
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
};
use sugarsubstitute_contracts::{execution::*, incident::*, lifecycle::NamespaceId};
use sugarsubstitute_crash::incident::{IncidentStoreState, encode_store_state, portable::*};
use sugarsubstitute_execution::ExecutionOwner;
use sugarsubstitute_supervision::diagnostic_storage::{DiagnosticEntryClass, DiagnosticObject};

type TestError = Box<dyn Error + Send + Sync>;
type TestResult = Result<(), TestError>;
struct Clock;
impl sugarsubstitute_contracts::execution::Clock for Clock {
    fn now(&self) -> Moment {
        Moment::from_nanos(1)
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
fn admitted(work: impl FnOnce(JobContext) -> TestResult + Send + 'static) -> TestResult {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(Clock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(100)),
            reservation_bytes: 32 * 1024 * 1024,
            replacement: None,
        },
        work,
    )?;
    loop {
        let prior = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(completion) = job.try_take_completion() {
            match completion.outcome {
                JobOutcome::Success(()) => break,
                JobOutcome::Failure(cause) => return Err(cause),
                outcome => {
                    return Err(format!("Public port worker did not succeed: {outcome:?}").into());
                }
            }
        }
        drop(
            wake.changed
                .wait_while(
                    wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
                    |count| *count == prior,
                )
                .map_err(|cause| io::Error::other(cause.to_string()))?,
        );
    }
    drop(job);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

struct Adapter {
    provenance: PortableIncidentProvenance,
    entries: Vec<PortableIncidentEntry>,
    ledger: Vec<u8>,
    fail: bool,
}
impl Adapter {
    fn new() -> Result<Self, TestError> {
        let namespace = NamespaceId::new([3; 32]);
        let ledger = encode_store_state(&IncidentStoreState::Ledger(SummaryEvictions::new(
            namespace, 0, 0, 0, None,
        )?))?;
        Ok(Self {
            provenance: PortableIncidentProvenance::new(namespace, [9; 32])?,
            entries: vec![PortableIncidentEntry::new(
                PortableIncidentEntryId::new(0)?,
                DiagnosticEntryClass::StoreState,
                ledger.len() as u64,
            )?],
            ledger,
            fail: false,
        })
    }
}
impl PortableIncidentReadSession for Adapter {
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
        if entry.get() != 0 {
            return Err(PortableIncidentError::rejected(
                PortableIncidentFailureCode::ChangedSource,
            ));
        }
        let offset = usize::try_from(offset)
            .map_err(|cause| PortableIncidentError::source(io::Error::other(cause)))?;
        if offset >= self.ledger.len() {
            return Ok(0);
        }
        let count = output.len().min(self.ledger.len() - offset);
        output[..count].copy_from_slice(&self.ledger[offset..offset + count]);
        Ok(count)
    }
    fn bind_typed_object(
        &mut self,
        entry: PortableIncidentEntryId,
        object: DiagnosticObject,
        _: &JobContext,
    ) -> Result<bool, PortableIncidentError> {
        Ok(entry.get() == 0 && object == DiagnosticObject::StoreState)
    }
    fn revalidate(&mut self, _: &JobContext) -> Result<(), PortableIncidentError> {
        if self.fail {
            return Err(PortableIncidentError::source(io::Error::other(
                "actual retained source failed",
            )));
        }
        Ok(())
    }
}

#[test]
fn public_adapter_qualifies_empty_committed_history_without_native_tokens() -> TestResult {
    admitted(|context| {
        let mut adapter = Adapter::new()?;
        let provenance = adapter.provenance();
        let history = project_portable_committed(
            &mut adapter,
            provenance.namespace(),
            IncidentUtc::from_unix_seconds(100)?,
            &context,
        )?;
        assert_eq!(history.provenance(), provenance);
        assert_eq!(history.summaries().len(), 0);
        Ok(())
    })
}
#[test]
fn public_adapter_retains_original_source_failure_and_rejects_foreign_selection() -> TestResult {
    admitted(|context| {
        let mut adapter = Adapter::new()?;
        let namespace = adapter.provenance().namespace();
        adapter.fail = true;
        let cause = match project_portable_committed(
            &mut adapter,
            namespace,
            IncidentUtc::from_unix_seconds(100)?,
            &context,
        ) {
            Ok(_) => return Err("Failed source accepted".into()),
            Err(cause) => cause,
        };
        assert_eq!(cause.code(), PortableIncidentFailureCode::Source);
        assert!(Error::source(&cause).is_some());
        drop(cause);
        adapter.fail = false;
        let cause = match project_portable_committed(
            &mut adapter,
            NamespaceId::new([4; 32]),
            IncidentUtc::from_unix_seconds(100)?,
            &context,
        ) {
            Ok(_) => return Err("Foreign selection accepted".into()),
            Err(cause) => cause,
        };
        assert_eq!(cause.code(), PortableIncidentFailureCode::ForeignNamespace);
        Ok(())
    })
}
