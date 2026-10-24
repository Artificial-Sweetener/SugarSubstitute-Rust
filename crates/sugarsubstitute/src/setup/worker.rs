// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind the provisioning transport to the existing retained Windows process-family owner.
use crate::job_sequence::JobSequence;
use std::{error::Error, path::PathBuf, time::Duration};
use sugarsubstitute_contracts::{
    execution::{
        CompletionBatch, ExecutionPort, JobClass, JobOutcome, JobSpec, Moment, Operation, OwnedJob,
        Scope, ScopeId, ScopeSpec,
    },
    lifecycle::{
        CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, ProcessKey, RunGeneration,
        RunKey, ShutdownBudget, ShutdownId,
    },
};
use sugarsubstitute_execution::{
    ExecutionClient,
    provisioning_transport::{self, Transport, TransportEvent},
};
use sugarsubstitute_supervision::{
    FamilyBinding, RegistryError, RegistryObservation, WindowsLaunchCommand, WindowsLaunchInput,
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
    executable: PathBuf,
    package: PathBuf,
    workspace: PathBuf,
    closing: bool,
    stopped: bool,
    ready: bool,
}

impl Worker {
    pub(super) fn start(
        client: ExecutionClient,
        scope: Scope,
        jobs: JobSequence,
        executable: PathBuf,
        package: PathBuf,
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
        let (transport, runner) = Transport::pair();
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
            executable,
            package,
            workspace,
            closing: false,
            stopped: false,
            ready: false,
        })
    }
    pub(super) fn ready(&self) -> bool {
        self.ready && !self.closing
    }
    pub(super) fn send(&self, value: &serde_json::Value) -> Result<()> {
        if !self.ready() {
            return Err("Provisioning worker is not ready".into());
        }
        self.transport
            .as_ref()
            .ok_or("Provisioning transport missing")?
            .send(serde_json::to_string(value)?)?;
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
        let mut events = Vec::new();
        if let Some(transport) = &self.transport {
            for _ in 0..16 {
                match transport.next_event() {
                    Some(TransportEvent::Listening { port, token }) => {
                        self.endpoint = Some((port, token))
                    }
                    Some(TransportEvent::Frame(frame)) => {
                        let value: serde_json::Value = serde_json::from_str(&frame)?;
                        if value["version"] != 1 {
                            return Err("Unsupported provisioning response".into());
                        }
                        if value["event"] == "ready" {
                            self.ready = true;
                        } else {
                            events.push(value);
                        }
                    }
                    None => break,
                }
            }
        }
        if !self.closing
            && self.process.is_none()
            && self.registry.is_some()
            && self.endpoint.is_some()
        {
            self.launch()?;
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
    fn launch(&mut self) -> Result<()> {
        let (port, token) = self.endpoint.take().ok_or("Provisioning endpoint absent")?;
        let cwd = self.package.parent().ok_or("Provisioning package has no parent")?;
        let arguments = [
            self.package.as_os_str().to_owned(),
            "--setup-workspace".into(),
            self.workspace.as_os_str().to_owned(),
            "--setup-port".into(),
            port.to_string().into(),
            "--setup-token".into(),
            token.into(),
        ];
        let command = WindowsLaunchCommand::from_os(&self.executable, cwd, &arguments)?;
        let environment =
            crate::worker_environment::capture().map_err(|cause| cause as Box<dyn Error>)?;
        let input = WindowsLaunchInput::new(command, environment)?;
        let watch = self.spec(JobClass::NativeObservation, 4096, None)?;
        let creation = self.spec(
            JobClass::InfrastructureLifecycle,
            input.required_reservation_bytes(),
            Some(self.client.now()?.checked_add(Duration::from_secs(30))?),
        )?;
        let binding = FamilyBinding::Desktop(RunKey {
            owner: self.identity,
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        });
        let ticket = self.registry.as_mut().ok_or("Provisioning registry missing")?.submit_launch(
            &self.client,
            &self.scope,
            ScopeSpec { id: ScopeId::new(3)?, deadline: None },
            creation,
            watch,
            binding,
            input,
        )?;
        self.process = Some(ticket.process());
        Ok(())
    }
    pub(super) fn close(&mut self) -> Result<()> {
        self.closing = true;
        self.ready = false;
        self.transport = None;
        if let Some(job) = &self.transport_job {
            job.cancel();
        }
        if let Some(registry) = &mut self.registry {
            registry.close_launch_admission();
        }
        self.cleanup()
    }
    fn cleanup(&mut self) -> Result<()> {
        let Some(process) = self.process else {
            return Ok(());
        };
        let now = self.client.now()?;
        if !self.stopped {
            self.registry.as_mut().ok_or("Provisioning registry missing")?.begin_stop(
                process,
                ShutdownId::new(1)?,
                ShutdownBudget::new(now)?,
            )?;
            self.stopped = true;
        }
        let snapshot =
            self.registry.as_ref().ok_or("Provisioning registry missing")?.snapshot(process)?;
        if snapshot.settled {
            self.registry
                .as_mut()
                .ok_or("Provisioning registry missing")?
                .retire_settled(process)?;
            self.process = None;
        } else if snapshot.cleanup_step_due(now) {
            let watch = if snapshot.member_watch_required {
                Some(self.spec(JobClass::NativeObservation, 4096, None)?)
            } else {
                None
            };
            let cleanup = self.spec(JobClass::InfrastructureLifecycle, 4096, None)?;
            self.registry.as_mut().ok_or("Provisioning registry missing")?.submit_cleanup_step(
                &self.client,
                process,
                &self.scope,
                cleanup,
                watch,
            )?;
        } else if snapshot.deadline_expired(now) {
            return Err("Provisioning family remains unsettled after cleanup deadline".into());
        }
        Ok(())
    }
    pub(super) fn settled(&self) -> bool {
        self.closing
            && self.creating.is_none()
            && self.transport_job.is_none()
            && self.registry.as_ref().is_none_or(WindowsProcessRegistry::is_empty)
    }
}
