// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify real storage jobs and bounded transfers over isolated owned directories.
mod baseline;
mod profiles;
mod storage;
mod transfers;
use super::*;
use crate::{completion_wake::Wake, job_sequence::JobSequence};
use std::sync::Arc;
use sugarsubstitute_contracts::{desktop_session::decode_checkpoint, execution::*};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner};
type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const VECTORS: &str =
    include_str!("../../../../third_party/sugarcubes-session/session_vectors.json");
fn checkpoint(number: u16) -> TestResult<SessionCheckpoint> {
    let vectors: serde_json::Value = serde_json::from_str(VECTORS)?;
    let mut value = vectors["checkpoints"][0]["value"].clone();
    value["id"] = format!("00000000-0000-4000-8000-{number:012x}").into();
    value["revision"] = u64::from(number).into();
    value["entries"] = serde_json::json!([]);
    value["activeEntry"] = serde_json::Value::Null;
    value["assets"] = serde_json::json!([]);
    Ok(decode_checkpoint(&serde_json::to_vec(&value)?)?)
}
struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
struct Fixture {
    wake: Arc<Wake>,
    client: ExecutionClient,
    scope: Scope,
    jobs: JobSequence,
    store: WorkspaceStore,
    directory: tempfile::TempDir,
    _owner: ExecutionOwner,
}
impl Fixture {
    fn new() -> TestResult<Self> {
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::with_memory_capacity(
            Arc::new(FixedClock),
            wake.clone(),
            std::num::NonZeroU64::new(3 * 1024 * 1024 * 1024).ok_or("Empty fixture admission")?,
        )?;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let directory = tempfile::tempdir()?;
        let store = WorkspaceStore::new(
            directory.path().join("workspace"),
            directory.path().join("assets"),
            checkpoint(1)?.scope,
        )?;
        Ok(Self { wake, client, scope, jobs: JobSequence::new(), store, directory, _owner: owner })
    }
    fn spec(&self) -> TestResult<JobSpec> {
        Ok(JobSpec {
            id: self.jobs.allocate()?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(Moment::from_nanos(60_000_000_000)),
            reservation_bytes: WorkspaceStore::reservation_bytes(),
            replacement: None,
        })
    }
    fn terminal<T, E>(&self, mut job: OwnedJob<T, E>) -> TestResult<JobOutcome<T, E>> {
        Ok(self.wake.completion(&self.client, &mut job)?.outcome)
    }
    fn save(&self, checkpoint: SessionCheckpoint) -> TestResult<Result<SaveReceipt, StoreError>> {
        let job = self.store.submit_save(&self.client, &self.scope, self.spec()?, checkpoint)?;
        match self.terminal(job)? {
            JobOutcome::Success(value) => Ok(Ok(value)),
            JobOutcome::Failure(cause) => Ok(Err(cause)),
            other => Err(format!("Save stopped: {:?}", other.kind()).into()),
        }
    }
    fn load(&self) -> TestResult<RestoredWorkspace> {
        let job = self.store.submit_load(&self.client, &self.scope, self.spec()?)?;
        match self.terminal(job)? {
            JobOutcome::Success(value) => Ok(value),
            JobOutcome::Failure(cause) => Err(cause.into()),
            other => Err(format!("Load stopped: {:?}", other.kind()).into()),
        }
    }
    fn path(&self, name: &str) -> PathBuf {
        self.store.directory().join(name)
    }
}
