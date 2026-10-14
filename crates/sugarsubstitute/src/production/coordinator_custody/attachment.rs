// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the original registered attachment before admitting the helper's captured bootstrap.
use super::{BackendResources, Result, spec};
use sugarsubstitute_contracts::{
    backend_custody::BackendCustodyKey,
    crash::CaptureTarget,
    execution::{JobClass, JobCompletion, JobOutcome, JobSpec, Moment, OwnedJob},
};
use sugarsubstitute_execution::{
    backend_custody::{
        BackendCustodyPipeReport, BackendCustodyPipeSession, MIN_LIFECYCLE_PIPE_BYTES,
    },
    windows_backend_custody::CapturedBackendCustodyStart,
    windows_capture_attach::{
        CaptureAttachFailure, CaptureAttachLaunchPin, CaptureAttachRetirement,
        CaptureAttachRetirementReport,
    },
    windows_lifecycle_pipe::LifecyclePipeFailure,
};
use sugarsubstitute_supervision::BackendCustodyParent;

/// Keep preparation delivery, retained bytes and native retirement with one genuine target.
pub(super) struct Attachment {
    key: BackendCustodyKey,
    until: Moment,
    job: Option<OwnedJob<CaptureAttachLaunchPin, CaptureAttachFailure>>,
    completion: Option<JobCompletion<Option<CaptureAttachLaunchPin>, CaptureAttachFailure>>,
    start: Option<CapturedBackendCustodyStart>,
    server: Option<BackendCustodyPipeSession>,
    server_spec: Option<JobSpec>,
    server_terminal: Option<JobCompletion<BackendCustodyPipeReport, LifecyclePipeFailure>>,
    retirement: Option<CaptureAttachRetirement>,
    retirement_report:
        Option<std::result::Result<CaptureAttachRetirementReport, CaptureAttachFailure>>,
    admitted: bool,
    retired: bool,
}
impl Attachment {
    /// Borrow the original registration; no independent handler or source inventory is created.
    pub(super) fn new(
        key: BackendCustodyKey,
        until: Moment,
        resources: &BackendResources<'_>,
    ) -> Result<Self> {
        if resources.client.now()? >= until {
            return Err("Original custody bootstrap expired before attachment admission".into());
        }
        let capture = resources.capture.ok_or("Actual custody capture registration absent")?;
        if !capture.registration_ready() {
            return Err("Actual custody capture registration is not ready".into());
        }
        let job = capture.attach(
            CaptureTarget::BackendCustody(key),
            resources.client,
            resources.native,
            resources.jobs,
        )?;
        Ok(Self {
            key,
            until,
            job: Some(job),
            completion: None,
            start: None,
            server: None,
            server_spec: None,
            server_terminal: None,
            retirement: None,
            retirement_report: None,
            admitted: false,
            retired: false,
        })
    }
    /// Preserve every original completion guard while transferring only its genuine pin.
    fn observe(&mut self) {
        if let Some(job) = &mut self.job
            && let Some(done) = job.try_take_completion()
        {
            self.job = None;
            self.completion = Some(JobCompletion {
                outcome: match done.outcome {
                    JobOutcome::Success(pin) => JobOutcome::Success(Some(pin)),
                    JobOutcome::Failure(cause) => JobOutcome::Failure(cause),
                    JobOutcome::Cancelled => JobOutcome::Cancelled,
                    JobOutcome::Panicked(cause) => JobOutcome::Panicked(cause),
                    JobOutcome::TimedOut => JobOutcome::TimedOut,
                    JobOutcome::Coalesced { replacement } => JobOutcome::Coalesced { replacement },
                },
                scope_id: done.scope_id,
                spec: done.spec,
                queue_time: done.queue_time,
                execution_time: done.execution_time,
                memory: done.memory,
                native_handles: done.native_handles,
            });
        }
    }
    /// Construct the sole captured v5 server after actual pin delivery and before helper launch.
    /// The original bootstrap cutoff is checked at each transfer; saturation never renews it.
    pub(super) fn advance(
        &mut self,
        resources: &BackendResources<'_>,
    ) -> Result<Option<BackendCustodyParent>> {
        if self.admitted {
            return Ok(None);
        }
        self.observe();
        if resources.client.now()? >= self.until {
            if let Some(job) = &self.job {
                job.cancel();
            }
            return Err("Original custody bootstrap expired during capture preparation".into());
        }
        let Some(completion) = &mut self.completion else { return Ok(None) };
        if completion.scope_id != resources.native.id()
            || completion.spec.class != JobClass::NativeObservation
        {
            return Err("Original custody attachment completion provenance differs".into());
        }
        match &mut completion.outcome {
            JobOutcome::Success(pin) => {
                if self.start.is_none() && self.server.is_none() {
                    self.start = Some(CapturedBackendCustodyStart::new(self.key, pin)?);
                }
            }
            JobOutcome::Failure(cause) => return Err(Box::new(cause.clone())),
            _ => return Err("Original custody attachment ended without a genuine pin".into()),
        }
        if self.server.is_none() {
            let job = spec(resources, JobClass::NativeObservation, MIN_LIFECYCLE_PIPE_BYTES, None)?;
            self.server = Some(resources.client.reserve_captured_backend_custody_pipe_server(
                resources.native,
                job,
                self.key,
                self.until,
                self.start.as_mut().ok_or("Original custody capture start absent")?,
            )?);
            self.server_spec = Some(job);
        }
        let parent = BackendCustodyParent::new(&mut self.server)?;
        self.admitted = true;
        Ok(Some(parent))
    }
    /// Cancel an unlaunched original attachment and observe its exact checked retirement.
    /// Admitted transport retirement is reported separately by its genuine parent owner.
    pub(super) fn close(&mut self, resources: &BackendResources<'_>) -> Result<bool> {
        if self.retired {
            return Ok(true);
        }
        if let Some(job) = &self.job {
            job.cancel();
        }
        self.observe();
        if self.job.is_some() {
            return Ok(false);
        }
        if let Some(server) = &mut self.server {
            server.control().close();
            if self.server_terminal.is_none() {
                self.server_terminal = server.take_terminal();
            }
            let Some(done) = &self.server_terminal else {
                return Ok(false);
            };
            if done.scope_id != resources.native.id() || Some(done.spec) != self.server_spec {
                return Err("Original unadopted custody transport completion differs".into());
            }
            match &done.outcome {
                JobOutcome::Success(actual) if actual.key.is_none_or(|key| key == self.key) => {}
                JobOutcome::Cancelled => {}
                JobOutcome::Failure(cause) => return Err(Box::new(cause.clone())),
                _ => return Err("Original unadopted custody transport retirement failed".into()),
            }
            self.server = None;
            self.retired = true;
            return Ok(true);
        }
        if self.admitted {
            return Ok(false);
        }
        let Some(completion) = &mut self.completion else { return Ok(false) };
        if self.retirement.is_none() {
            match &mut completion.outcome {
                JobOutcome::Success(pin) => {
                    self.retirement =
                        pin.take().map(CaptureAttachLaunchPin::release).or_else(|| {
                            self.start.as_mut().and_then(CapturedBackendCustodyStart::release)
                        });
                }
                JobOutcome::Failure(cause) => return Err(Box::new(cause.clone())),
                JobOutcome::Cancelled => {
                    self.retired = true;
                    return Ok(true);
                }
                _ => return Err("Original custody attachment cleanup failed".into()),
            }
        }
        if self.retirement_report.is_none()
            && let Some(observer) = &mut self.retirement
        {
            self.retirement_report = observer.try_take_report();
        }
        let actual = match &self.retirement_report {
            Some(Ok(actual)) => actual,
            Some(Err(cause)) => return Err(Box::new(cause.clone())),
            None => return Ok(false),
        };
        if actual.target() != CaptureTarget::BackendCustody(self.key)
            || actual.execution_id() != resources.native.owner_id()
            || actual.job_id() != completion.spec.id
        {
            return Err("Actual custody attachment retirement provenance differs".into());
        }
        self.retired = true;
        Ok(true)
    }
    /// Record only the actual parent transport terminal after helper/family retirement.
    pub(super) fn parent_retired(&mut self) {
        if self.admitted {
            self.retired = true;
        }
    }
}
