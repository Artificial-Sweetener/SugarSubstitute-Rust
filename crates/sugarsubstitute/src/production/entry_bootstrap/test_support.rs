// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own isolated files and synchronize actual terminal publication without sleeps.
use super::*;
use std::{
    num::NonZeroUsize,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::{
    Clock, CompletionWake, ExecutionError, ExecutionPort, ScopeId, ScopeSpec, WaitFuture,
};
use sugarsubstitute_execution::ExecutionOwner;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);
pub(super) struct Files {
    directory: PathBuf,
}
impl Files {
    pub(super) fn new() -> Result<Self, Box<dyn Error>> {
        let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir()
            .join(format!("sugarsubstitute-entry-bootstrap-{}-{id}", std::process::id()));
        std::fs::create_dir(&directory)?;
        std::fs::write(directory.join("startup.toml"), b"invalid startup configuration")?;
        Ok(Self { directory })
    }
    pub(super) fn bootstrap(&self) -> Result<EntryBootstrap, Box<dyn Error>> {
        Ok(EntryBootstrap::new(
            ProductionEntryOptions {
                configuration: self.directory.join("startup.toml"),
                package: self.directory.join("frontend"),
                metadata: self.directory.join("METADATA"),
                cubes: self.directory.join("SugarCubes"),
                script: self.directory.join("main.py"),
                profile: self.directory.join("profile"),
            },
            self.directory.clone(),
            self.directory.join("SugarSubstitute.exe"),
            EntrySetupBudget::new(Moment::from_nanos(0))?,
        )?)
    }
    pub(super) fn finish(self) -> Result<(), std::io::Error> {
        std::fs::remove_file(self.directory.join("startup.toml"))?;
        std::fs::remove_dir(self.directory)
    }
}
struct FrozenClock;
impl Clock for FrozenClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
struct Wake {
    revision: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let mut revision = self.revision.lock().map_err(|_| {
            ExecutionError::new(
                Operation::Infrastructure,
                Box::new(std::io::Error::other("Test wake poisoned")),
            )
        })?;
        *revision = revision.saturating_add(1);
        self.changed.notify_all();
        Ok(())
    }
}
/// The only threads/pools are the real execution owner's fixed workers.
pub(super) struct Runtime {
    pub(super) client: ExecutionClient,
    wake: Arc<Wake>,
    _owner: ExecutionOwner,
}
impl Runtime {
    pub(super) fn new() -> Result<Self, Box<dyn Error>> {
        let wake = Arc::new(Wake { revision: Mutex::new(0), changed: Condvar::new() });
        let owner = ExecutionOwner::new(Arc::new(FrozenClock), wake.clone())?;
        Ok(Self { client: owner.client(), wake, _owner: owner })
    }
    pub(super) fn scope(&self, id: u64) -> Result<Scope, Box<dyn Error>> {
        Ok(self.client.create_scope(ScopeSpec { id: ScopeId::new(id)?, deadline: None }, None)?)
    }
    /// The wall-clock cutoff bounds hangs; all application time stays frozen.
    pub(super) fn failure(
        &self,
        bootstrap: &mut EntryBootstrap,
        jobs: &JobSequence,
    ) -> Result<EntryBootstrapIssue, Box<dyn Error>> {
        let until =
            Instant::now().checked_add(Duration::from_secs(30)).ok_or("Test cutoff overflow")?;
        loop {
            let revision = *self.wake.revision.lock().map_err(|_| "Test wake poisoned")?;
            let batch = self.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
            if let Err(cause) = bootstrap.consume(&batch, jobs) {
                return Ok(cause);
            }
            let mut observed = self.wake.revision.lock().map_err(|_| "Test wake poisoned")?;
            while *observed <= revision {
                let remaining = until
                    .checked_duration_since(Instant::now())
                    .ok_or("Actual job did not publish")?;
                let (next, timed) = self
                    .wake
                    .changed
                    .wait_timeout(observed, remaining)
                    .map_err(|_| "Test wake poisoned")?;
                observed = next;
                if timed.timed_out() && *observed <= revision {
                    return Err("Actual job did not publish".into());
                }
            }
        }
    }
}
