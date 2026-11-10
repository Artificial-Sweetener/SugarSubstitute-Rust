// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coordinate one installer transaction through admitted storage jobs and the retained worker.
mod completion;
mod events;
use events::{Completed, Event, Kind, Progress};
mod lifetime;
mod native;
mod navigation;
pub(super) use native::Native;
mod provisioning;
mod session;
mod storage;
use storage::{StorageJob, StorageResult, StorageWork};
pub(super) mod managed_qualification;
pub(super) mod qualification;
mod requests;
use super::worker::Worker;
use crate::{job_sequence::JobSequence, scope_sequence::ScopeSequence};
pub(super) use native::qualification as window_qualification;
use serde_json::{Value, json};
use std::{collections::VecDeque, error::Error, io, path::PathBuf, sync::Arc};
use sugarsubstitute_contracts::execution::{
    Clock, ExecutionPort, JobClass, JobContext, JobOutcome, JobSpec, Operation, OwnedJob, Scope,
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

/// One root drain retains rendering metadata alongside the corresponding backend results.
struct Advance {
    completions: sugarsubstitute_contracts::execution::CompletionBatch,
    events: Vec<Event>,
}

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
    clock: Arc<dyn Clock>,
    owner: Option<ExecutionOwner>,
    client: Option<ExecutionClient>,
    scope: Option<Scope>,
    jobs: JobSequence,
    scopes: ScopeSequence,
    storage: Option<StorageJob>,
    bundle: Option<Arc<VerifiedBundle>>,
    cache: Option<setup_storage::cache::RuntimeCache>,
    prepared: Option<Box<SignedPreparation>>,
    integration: Option<Box<setup_storage::IntegrationStorage>>,
    installed: Option<CompletedInstallation>,
    relaunch: Option<ApplicationLaunch>,
    launch_failure: Option<io::Error>,
    worker: Option<Worker>,
    pending: Option<Pending>,
    waiting: Option<requests::Request>,
    selection: Option<Selection>,
    install_id: Option<String>,
    inspection_id: Option<String>,
    output: VecDeque<Event>,
    closing: bool,
    stopping: bool,
    unavailable: bool,
    console: Option<super::console::Output>,
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
            scopes: ScopeSequence::new(),
            storage: None,
            bundle: None,
            cache: None,
            prepared: None,
            integration: None,
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
            locale: "en".into(),
        }
    }
}

impl Controller {
    /// Borrow the validated locale after native retirement for terminal feedback.
    pub(super) fn locale(&self) -> &str {
        &self.locale
    }
}
