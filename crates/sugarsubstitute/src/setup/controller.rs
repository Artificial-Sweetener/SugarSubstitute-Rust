// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coordinate one installer transaction through admitted storage jobs and the retained worker.
mod completion;
mod lifetime;
mod provisioning;
mod storage;
use storage::{StorageJob, StorageResult, StorageWork};
pub(super) mod managed_qualification;
pub(super) mod qualification;
mod requests;
pub(super) mod window_qualification;
use super::worker::Worker;
use crate::job_sequence::JobSequence;
use serde_json::{Value, json};
use std::{collections::VecDeque, error::Error, io, path::PathBuf, sync::Arc};
use sugarsubstitute_contracts::execution::{
    ExecutionPort, JobClass, JobContext, JobOutcome, JobSpec, Operation, OwnedJob, Scope, ScopeId,
    ScopeSpec,
};
use sugarsubstitute_execution::{ExecutionClient, ExecutionOwner, MonotonicClock};
use sugarsubstitute_supervision::{
    application_launch::{ApplicationLaunch, CreationOutcome},
    setup_storage::{
        self, CompletedInstallation, RuntimePaths, Selection, SignedCandidate, SignedPreparation,
        VerifiedBundle,
    },
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

enum Purpose {
    Options,
    Inspect,
    Provision,
    Integrate,
}
struct Pending {
    id: String,
    purpose: Purpose,
}

pub(super) struct Controller {
    distribution: &'static [u8],
    bundle_root: PathBuf,
    workspace: PathBuf,
    clock: Arc<MonotonicClock>,
    owner: Option<ExecutionOwner>,
    client: Option<ExecutionClient>,
    scope: Option<Scope>,
    jobs: JobSequence,
    storage: Option<StorageJob>,
    bundle: Option<Arc<VerifiedBundle>>,
    prepared: Option<Box<SignedPreparation>>,
    installed: Option<CompletedInstallation>,
    relaunch: Option<ApplicationLaunch>,
    launch_failure: Option<io::Error>,
    worker: Option<Worker>,
    pending: Option<Pending>,
    waiting: Option<Value>,
    selection: Option<Selection>,
    install_id: Option<String>,
    inspection_id: Option<String>,
    output: VecDeque<Value>,
    closing: bool,
    stopping: bool,
    unavailable: bool,
    console: Option<super::console::Output>,
    profile_job: Option<OwnedJob<setup_storage::InstallerProfile, io::Error>>,
    profile: Option<setup_storage::InstallerProfile>,
    locale: String,
}

impl Controller {
    pub(super) fn new(
        bundle_root: PathBuf,
        workspace: PathBuf,
        distribution: &'static [u8],
    ) -> Self {
        Self {
            distribution,
            bundle_root,
            workspace,
            clock: MonotonicClock::new(),
            owner: None,
            client: None,
            scope: None,
            jobs: JobSequence::new(),
            storage: None,
            bundle: None,
            prepared: None,
            installed: None,
            relaunch: None,
            launch_failure: None,
            worker: None,
            pending: None,
            waiting: None,
            selection: None,
            install_id: None,
            inspection_id: None,
            output: VecDeque::new(),
            closing: false,
            stopping: false,
            unavailable: false,
            console: None,
            profile_job: None,
            profile: None,
            locale: "en".into(),
        }
    }
    fn reply(&mut self, id: Option<&str>, event: &str, data: Value) -> Result<()> {
        if matches!(event, "progress" | "diagnostic" | "unavailable" | "failed")
            && let Some(console) = &mut self.console
            && let Some(text) = data["message"].as_str().or_else(|| data["status"].as_str())
        {
            console.append(text)?;
        }
        if self.output.len() >= 32 {
            return Err("Installer response admission exceeded".into());
        }
        self.output.push_back(json!({"version":1,"id":id,"event":event,"data":data}));
        Ok(())
    }
}
