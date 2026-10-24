// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind the provisioning transport to the existing retained Windows process-family owner.
mod cache_jobs;
mod downloads;
mod metadata;
mod process;
mod protocol;
mod records;
use crate::job_sequence::JobSequence;
use cache_jobs::CacheFinish;
use std::{error::Error, path::PathBuf, sync::Arc, time::Duration};
use sugarsubstitute_contracts::{
    execution::{
        CompletionBatch, ExecutionPort, JobClass, JobOutcome, JobSpec, Moment, Operation, OwnedJob,
        Scope, ScopeId, ScopeSpec,
    },
    lifecycle::{
        CoordinatorEpoch, CoordinatorKey, NamespaceId, ProcessKey, ShutdownBudget, ShutdownId,
    },
};
use sugarsubstitute_execution::{
    ExecutionClient,
    provisioning_transport::{self, Transport, TransportEvent},
};
use sugarsubstitute_supervision::{
    FamilyBinding, RegistryError, RegistryObservation, SettledProvisioning, WindowsLaunchInput,
    WindowsProcessRegistry,
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub(super) struct Worker {
    registry: Option<WindowsProcessRegistry>,
    creating: Option<OwnedJob<WindowsProcessRegistry, RegistryError>>,
    transport: Option<Transport>,
    transport_job: Option<OwnedJob<(), std::io::Error>>,
    process: Option<ProcessKey>,
    endpoint: Option<(u16, String)>,
    identity: CoordinatorKey,
    client: ExecutionClient,
    scope: Scope,
    jobs: JobSequence,
    bundle: Arc<sugarsubstitute_supervision::setup_storage::VerifiedBundle>,
    cache: sugarsubstitute_supervision::setup_storage::cache::RuntimeCache,
    preparing: Option<OwnedJob<WindowsLaunchInput, std::io::Error>>,
    cache_job: Option<OwnedJob<CacheFinish, std::io::Error>>,
    retired: Option<SettledProvisioning>,
    cache_finalized: bool,
    workspace: PathBuf,
    closing: bool,
    stopped: bool,
    ready: bool,
    pressure: sugarsubstitute_supervision::setup_storage::ProvisioningCachePressure,
    record_scope: Option<records::RecordScope>,
    record_state: Option<records::RecordSession>,
    record_job: Option<OwnedJob<records::RecordCompletion, std::io::Error>>,
    metadata_access: Option<sugarsubstitute_supervision::setup_storage::ProvisioningMetadata>,
    metadata_scope: Option<metadata::MetadataScope>,
    metadata_state: Option<metadata::session::Session>,
    metadata_io: Option<OwnedJob<metadata::MetadataCompletion, std::io::Error>>,
    downloads: Option<downloads::Session>,
    download_io: Option<OwnedJob<downloads::Batch, std::io::Error>>,
    download_terminal: Option<provisioning_transport::Frame>,
}

impl Worker {
    pub(super) fn start(
        client: ExecutionClient,
        scope: Scope,
        jobs: JobSequence,
        bundle: Arc<sugarsubstitute_supervision::setup_storage::VerifiedBundle>,
        cache: sugarsubstitute_supervision::setup_storage::cache::RuntimeCache,
        workspace: PathBuf,
    ) -> Result<Self> {
        // These local matching facts grant no external authority. The socket's
        // independent random capability authenticates the launched worker.
        let identity = CoordinatorKey {
            namespace: NamespaceId::new([0x49; 32]),
            epoch: CoordinatorEpoch::new([0x50; 16])?,
        };
        let now = client.now()?;
        let specification = JobSpec {
            id: jobs.allocate()?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(now.checked_add(Duration::from_secs(30))?),
            reservation_bytes: WindowsProcessRegistry::required_reservation_bytes(),
            replacement: None,
        };
        let creating =
            WindowsProcessRegistry::submit_new(&client, &scope, specification, identity)?;
        let (mut transport, runner) = Transport::pair();
        let downloads = downloads::Session::new(transport.take_downloads()?);
        let transport_job = client.submit_async(
            &scope,
            JobSpec {
                id: jobs.allocate()?,
                operation: Operation::Infrastructure,
                class: JobClass::IoOrdinary,
                deadline: None,
                reservation_bytes: provisioning_transport::RESERVATION_BYTES,
                replacement: None,
            },
            move |context| runner.run(context),
        )?;
        Ok(Self {
            registry: None,
            creating: Some(creating),
            transport: Some(transport),
            transport_job: Some(transport_job),
            process: None,
            endpoint: None,
            identity,
            client,
            scope,
            jobs,
            bundle,
            cache,
            preparing: None,
            cache_job: None,
            retired: None,
            cache_finalized: false,
            workspace,
            closing: false,
            stopped: false,
            ready: false,
            pressure: Default::default(),
            record_scope: None,
            record_state: None,
            record_job: None,
            metadata_access: None,
            metadata_scope: None,
            metadata_state: None,
            metadata_io: None,
            downloads: Some(downloads),
            download_io: None,
            download_terminal: None,
        })
    }
    pub(super) fn ready(&self) -> bool {
        self.ready && !self.closing
    }
    pub(super) fn send(&mut self, value: &serde_json::Value) -> Result<()> {
        if !self.ready() {
            return Err("Provisioning worker is not ready".into());
        }
        let authority = self.record_authority(value)?;
        let metadata = metadata::command_scope(value)?;
        if value["operation"] != "cancel" && self.metadata_io.is_some() {
            return Err("Native metadata operation is still active".into());
        }
        if value["operation"] != "cancel" {
            self.begin_downloads(value)?;
        }
        self.transport
            .as_ref()
            .ok_or("Provisioning transport missing")?
            .send(serde_json::to_string(value)?)?;
        if value["operation"] != "cancel" {
            self.metadata_state = None;
            self.metadata_scope = metadata;
            self.record_state = None;
            self.record_scope = authority;
        }
        Ok(())
    }
    pub(super) fn advance(&mut self, batch: &CompletionBatch) -> Result<Vec<serde_json::Value>> {
        if let Some(job) = &mut self.creating
            && let Some(completion) = job.try_take_completion()
        {
            self.creating = None;
            match completion.outcome {
                JobOutcome::Success(registry) => self.registry = Some(registry),
                JobOutcome::Failure(cause) => return Err(Box::new(cause)),
                _ => return Err("Installer process registry did not initialize".into()),
            }
        }
        if let Some(registry) = &mut self.registry {
            registry.consume(batch)?;
        }
        self.poll_cache_jobs()?;
        self.poll_record_job()?;
        self.poll_metadata_io()?;
        self.poll_download_io()?;
        let mut events = Vec::new();
        for _ in 0..16 {
            match self.next_worker_event() {
                Some(TransportEvent::Listening { port, token }) => {
                    self.endpoint = Some((port, token))
                }
                Some(TransportEvent::Frame(frame)) => {
                    let mut value: serde_json::Value = serde_json::from_str(frame.as_str())?;
                    if self.downloads.as_ref().is_some_and(|session| session.terminal(&value)) {
                        if self.download_terminal.is_some() {
                            return Err("Duplicate native download terminal result".into());
                        }
                        if let Some(session) = &self.downloads {
                            session.stop();
                        }
                        self.download_terminal = Some(frame);
                        continue;
                    }
                    if self.metadata_event(&value)? {
                        continue;
                    }
                    if self.record_event(&value)? {
                        continue;
                    }
                    if matches!(value["event"].as_str(), Some("failed" | "fatal"))
                        && (self.pressure.downloads || self.pressure.metadata)
                    {
                        value
                            .get_mut("data")
                            .and_then(serde_json::Value::as_object_mut)
                            .ok_or("Provisioning failure data is invalid")?
                            .insert("statusCode".into(), serde_json::json!("cacheFull"));
                    }
                    match protocol::cache_event(&value)? {
                        protocol::CacheEvent::Ready => self.ready = true,
                        protocol::CacheEvent::DownloadsFull => self.pressure.downloads = true,
                        protocol::CacheEvent::MetadataFull => self.pressure.metadata = true,
                        protocol::CacheEvent::Other => events.push(value),
                    }
                }
                None => break,
            }
        }
        if !self.closing
            && self.process.is_none()
            && self.preparing.is_none()
            && self.registry.is_some()
            && self.endpoint.is_some()
        {
            self.prepare_input()?;
        }
        if let Some(job) = &mut self.transport_job
            && let Some(completion) = job.try_take_completion()
        {
            self.transport_job = None;
            match completion.outcome {
                JobOutcome::Success(()) | JobOutcome::Cancelled if self.closing => {}
                JobOutcome::Failure(cause) => return Err(Box::new(cause)),
                _ => return Err("Provisioning worker disconnected unexpectedly".into()),
            }
        }
        if let (Some(registry), Some(process)) = (&mut self.registry, self.process) {
            for _ in 0..5 {
                match registry.take_observation(process)? {
                    Some(RegistryObservation::Failed { error, .. }) => return Err(Box::new(error)),
                    Some(RegistryObservation::RootExited { .. }) if !self.closing => {
                        return Err("Provisioning worker exited unexpectedly".into());
                    }
                    Some(_) => {}
                    None => break,
                }
            }
        }
        if self.closing {
            self.cleanup()?;
        }
        Ok(events)
    }
    fn spec(&self, class: JobClass, bytes: u64, deadline: Option<Moment>) -> Result<JobSpec> {
        Ok(JobSpec {
            id: self.jobs.allocate()?,
            operation: Operation::NativeLifecycle,
            class,
            deadline,
            reservation_bytes: bytes,
            replacement: None,
        })
    }
    pub(super) fn close(&mut self) -> Result<()> {
        self.closing = true;
        self.ready = false;
        if let Some(job) = &self.transport_job {
            job.cancel();
        }
        self.transport = None;
        self.record_scope = None;
        self.record_state = None;
        self.metadata_access = None;
        self.metadata_scope = None;
        self.metadata_state = None;
        self.download_terminal = None;
        if let Some(session) = &self.downloads {
            session.stop();
        }
        if self.download_io.is_none() {
            if self.downloads.as_ref().is_some_and(|session| session.active()) {
                self.submit_download_batch()?;
            } else {
                self.downloads = None;
            }
        }
        if let Some(job) = &self.metadata_io {
            job.cancel();
        }
        if let Some(job) = &self.record_job {
            job.cancel();
        }
        if let Some(job) = &self.preparing {
            job.cancel();
        }
        if let Some(registry) = &mut self.registry {
            registry.close_launch_admission();
        }
        self.cleanup()
    }
    pub(super) fn settled(&self) -> bool {
        self.closing
            && self.creating.is_none()
            && self.preparing.is_none()
            && self.cache_job.is_none()
            && self.retired.is_none()
            && self.cache_finalized
            && self.transport_job.is_none()
            && self.record_job.is_none()
            && self.metadata_io.is_none()
            && self.download_io.is_none()
            && self.downloads.is_none()
            && self.registry.as_ref().is_none_or(WindowsProcessRegistry::is_empty)
    }
}
