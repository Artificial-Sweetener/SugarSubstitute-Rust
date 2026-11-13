// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe one native download service per host command without sending payload bytes through UI ticks.
#[cfg(test)]
mod native_tests;
mod service;
mod wire;
use super::*;
use std::{
    collections::BTreeSet,
    io,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use sugarsubstitute_execution::provisioning_transport::DownloadPort;
use sugarsubstitute_supervision::setup_storage::ProvisioningDownloads;

/// Keep the original native port and prepared grant outside discardable job results.
struct Native {
    port: DownloadPort,
    access: Option<ProvisioningDownloads>,
    active: BTreeSet<String>,
    serial: u64,
}
/// A finite batch yields ordinary capacity while keeping command custody with the session.
#[derive(Debug, PartialEq)]
pub(super) enum Batch {
    /// Writer custody and command ordering remain retained for the next admitted batch.
    Continue,
    /// Every active writer has flushed and closed after the command's explicit stop request.
    Stopped,
}
/// Current command's matching identity and explicit graceful service-stop request.
struct Command {
    id: String,
    stop: Arc<AtomicBool>,
}
/// UI-owned control; filesystem work and blocking receive remain on the admitted service job.
pub(super) struct Session {
    native: Arc<Mutex<Native>>,
    pressure: Arc<AtomicBool>,
    command: Option<Command>,
}
impl Session {
    /// Taking a concrete transport port creates no worker, socket or native authority.
    pub(super) fn new(port: DownloadPort) -> Self {
        Self {
            native: Arc::new(Mutex::new(Native {
                port,
                access: None,
                active: BTreeSet::new(),
                serial: 0,
            })),
            pressure: Arc::new(AtomicBool::new(false)),
            command: None,
        }
    }
    /// Bind only the actual prepared launch's native cache capability before starting the worker.
    pub(super) fn bind(&self, access: ProvisioningDownloads) -> io::Result<()> {
        let mut native = self.native.try_lock().map_err(|cause| match cause {
            std::sync::TryLockError::WouldBlock => {
                io::Error::new(io::ErrorKind::WouldBlock, "Native download service active")
            }
            std::sync::TryLockError::Poisoned(_) => {
                io::Error::other("Native download custody poisoned")
            }
        })?;
        if native.access.is_some() {
            return Err(io::Error::other("Download grant already bound"));
        }
        native.access = Some(access);
        Ok(())
    }
    /// Match only this host command's terminal result; wire identities never create native access.
    pub(super) fn terminal(&self, value: &serde_json::Value) -> bool {
        self.command.as_ref().is_some_and(|command| {
            value["id"].as_str() == Some(&command.id)
                && matches!(
                    value["event"].as_str(),
                    Some("completed" | "failed" | "cancelled" | "fatal")
                )
        })
    }
    /// Request normal observed completion while keeping the native port and grant retained.
    pub(super) fn stop(&self) {
        if let Some(command) = &self.command {
            command.stop.store(true, Ordering::Release);
        }
    }
    /// Keep command custody after a failed batch admission until checked cleanup is observed.
    pub(super) fn active(&self) -> bool {
        self.command.is_some()
    }
}
impl Worker {
    /// Begin an observed command before publishing its host command, then admit a finite batch.
    pub(super) fn begin_downloads(&mut self, value: &serde_json::Value) -> Result<()> {
        if self.download_io.is_some() || self.download_terminal.is_some() {
            return Err("Native download command remains active".into());
        }
        let session = self.downloads.as_mut().ok_or("Native downloads absent")?;
        if session.command.is_some() {
            return Err("Native download command not observed".into());
        }
        let id = value["id"]
            .as_str()
            .filter(|id| !id.is_empty() && id.len() <= 256)
            .ok_or("Download command identity invalid")?
            .to_owned();
        let stop = Arc::new(AtomicBool::new(false));
        {
            let mut native =
                session.native.try_lock().map_err(|_| "Native download custody busy")?;
            if !native.active.is_empty() {
                return Err("Native download writers remain unobserved".into());
            }
            native.serial = 0;
        }
        session.command = Some(Command { id, stop });
        if let Err(cause) = self.submit_download_batch() {
            if let Some(session) = &mut self.downloads {
                session.command = None;
            }
            return Err(cause);
        }
        Ok(())
    }
    /// Yield the ordinary blocking slot after at most sixty-four requests or one idle wait.
    pub(super) fn submit_download_batch(&mut self) -> Result<()> {
        let mut specification = self.spec(
            JobClass::InfrastructureOrdinary,
            10 * 1024 * 1024,
            Some(self.client.now()?.checked_add(Duration::from_secs(30))?),
        )?;
        specification.operation = Operation::Infrastructure;
        let session = self.downloads.as_ref().ok_or("Native downloads absent")?;
        let command = session.command.as_ref().ok_or("Native download command absent")?;
        let native = session.native.clone();
        let pressure = session.pressure.clone();
        let running_stop = command.stop.clone();
        let request = command.id.clone();
        self.download_io =
            Some(self.client.submit_blocking(&self.scope, specification, move |context| {
                service::run(native, running_stop, pressure, request, context)
            })?);
        Ok(())
    }
    /// Observe actual job settlement before releasing a terminal result or allowing cache cleanup.
    pub(super) fn poll_download_io(&mut self) -> Result<()> {
        if let Some(session) = &self.downloads {
            self.pressure.downloads |= session.pressure.load(Ordering::Acquire);
        }
        let Some(job) = &mut self.download_io else {
            if self.closing {
                if self.downloads.as_ref().is_some_and(Session::active) {
                    return self.submit_download_batch();
                }
                self.downloads = None;
            }
            return Ok(());
        };
        let Some(completion) = job.try_take_completion() else {
            return Ok(());
        };
        self.download_io = None;
        let outcome = completion.outcome;
        drop(completion.memory);
        match outcome {
            JobOutcome::Success(Batch::Continue) => return self.submit_download_batch(),
            JobOutcome::Success(Batch::Stopped) => {}
            JobOutcome::Failure(cause)
                if self.closing
                    && matches!(
                        cause.kind(),
                        io::ErrorKind::BrokenPipe | io::ErrorKind::Interrupted
                    ) => {}
            JobOutcome::Failure(cause) => return Err(cause.into()),
            JobOutcome::Panicked(cause) => {
                return Err(io::Error::other(format!(
                    "Native download service panicked: {}",
                    cause.as_str()
                ))
                .into());
            }
            JobOutcome::Cancelled => {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Native download service cancelled before command completion",
                )
                .into());
            }
            JobOutcome::TimedOut => {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Native download service deadline expired",
                )
                .into());
            }
            JobOutcome::Coalesced { replacement } => {
                return Err(io::Error::other(format!(
                    "Native download service unexpectedly replaced by {replacement:?}"
                ))
                .into());
            }
        }
        if let Some(session) = &mut self.downloads {
            session.command = None;
        }
        if self.closing {
            self.downloads = None;
        }
        Ok(())
    }
    /// Replay a held terminal frame only after its native storage service actually completes.
    pub(super) fn next_worker_event(&mut self) -> Option<TransportEvent> {
        if self.download_io.is_none()
            && !self.downloads.as_ref().is_some_and(Session::active)
            && let Some(frame) = self.download_terminal.take()
        {
            return Some(TransportEvent::Frame(frame));
        }
        self.transport.as_ref().and_then(Transport::next_event)
    }
}
