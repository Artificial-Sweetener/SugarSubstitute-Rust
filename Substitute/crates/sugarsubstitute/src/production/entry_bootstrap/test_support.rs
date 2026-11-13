// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own isolated files and synchronize actual terminal publication without sleeps.
use super::*;
use crate::completion_wake::Wake;
use std::{
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use sugarsubstitute_configuration::paths::StartupPaths;
use sugarsubstitute_contracts::execution::{Clock, ExecutionPort, ScopeId, ScopeSpec, WaitFuture};
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
            StartupPaths::new([
                self.directory.join("startup.toml"),
                self.directory.join("frontend"),
                self.directory.join("METADATA"),
                self.directory.join("SugarCubes"),
                self.directory.join("main.py"),
                self.directory.join("profile"),
            ])?
            .into(),
            self.directory.clone(),
            sugarsubstitute_supervision::coordinator_entry::DiagnosticStorageLocation::Account,
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
/// The only threads/pools are the real execution owner's fixed workers.
pub(super) struct Runtime {
    pub(super) client: ExecutionClient,
    wake: Arc<Wake>,
    _owner: ExecutionOwner,
}
impl Runtime {
    pub(super) fn new() -> Result<Self, Box<dyn Error>> {
        let wake = Arc::new(Wake::default());
        let owner = ExecutionOwner::new(Arc::new(FrozenClock), wake.clone())?;
        Ok(Self { client: owner.client(), wake, _owner: owner })
    }
    pub(super) fn scope(&self, id: u64) -> Result<Scope, Box<dyn Error>> {
        Ok(self.client.create_scope(ScopeSpec { id: ScopeId::new(id)?, deadline: None }, None)?)
    }
    /// Deliver completions to `bootstrap` until it reports its first failure; all application
    /// time stays frozen and the wait ends only on the execution owner's wake.
    pub(super) fn failure(
        &self,
        bootstrap: &mut EntryBootstrap,
        jobs: &JobSequence,
    ) -> Result<EntryBootstrapIssue, Box<dyn Error>> {
        self.wake.until(|| {
            let batch = match self.client.drain_completions(NonZeroUsize::MIN.saturating_add(31)) {
                Ok(batch) => batch,
                Err(cause) => return Some(Err(cause.into())),
            };
            bootstrap.consume(&batch, jobs).err().map(Ok)
        })
    }
}
