// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Supervise post-acceptance shell publication without coupling failure to application health.
use super::*;
use std::{io, time::Duration};

/// One optional publication attempt, retaining admission failure or the actual worker terminal.
/// Cancellation and expiry drain normally and cannot invalidate accepted application health.
pub(super) struct Shell {
    issued: bool,
    job: Option<OwnedJob<(), io::Error>>,
    completion: Option<terminal::Terminal>,
    admission: Option<io::Error>,
}
impl Shell {
    /// Create inert state; application readiness has not authorized shell publication yet.
    pub(super) fn new() -> Self {
        Self { issued: false, job: None, completion: None, admission: None }
    }
    /// A rejected admission is an observed attempt, never an event-loop retry storm.
    pub(super) fn unissued(&self) -> bool {
        !self.issued
    }
    /// Native work must actually return before coordinator custody can retire.
    pub(super) fn pending(&self) -> bool {
        self.job.is_some()
    }

    /// Start once after health acceptance; keep failures observable without closing the app.
    pub(super) fn start(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        application: Arc<RunningApplication>,
    ) {
        if self.issued {
            return;
        }
        self.issued = true;
        match Self::admit(client, scope, jobs, application) {
            Ok(job) => self.job = Some(job),
            Err(cause) => {
                tracing::warn!(operation="installation_shell_admission_failed", cause=%crate::failure_details::Details(&cause),
                    "Application is healthy; shell publication could not start");
                self.admission = Some(cause);
            }
        }
    }
    /// The deadline bounds optional shell work after readiness, never ComfyUI provisioning.
    fn admit(
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        application: Arc<RunningApplication>,
    ) -> io::Result<OwnedJob<(), io::Error>> {
        let spec = JobSpec {
            id: jobs.allocate().map_err(io::Error::other)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(
                client
                    .now()
                    .map_err(io::Error::other)?
                    .checked_add(Duration::from_secs(30))
                    .map_err(io::Error::other)?,
            ),
            reservation_bytes: setup_storage::LAUNCH_RESERVATION_BYTES,
            replacement: None,
        };
        client
            .submit_blocking(scope, spec, move |context| {
                application.repair_launch_entry(&context).map(|_| ()).map_err(io::Error::other)
            })
            .map_err(io::Error::other)
    }

    /// Observe every outcome and retain its cause. Only mismatched completion identity is fatal.
    pub(super) fn consume(&mut self, batch: &CompletionBatch) -> Result<(), AuthorityIssue> {
        let Some(job) = &mut self.job else {
            return Ok(());
        };
        let Some(notice) = batch
            .notifications
            .iter()
            .find(|notice| (notice.scope_id, notice.job_id) == (job.scope_id(), job.spec().id))
        else {
            return Ok(());
        };
        let Some(completion) = job.try_take_completion() else {
            return Ok(());
        };
        let valid = completion.scope_id == job.scope_id()
            && completion.spec == job.spec()
            && completion.outcome.kind() == notice.kind;
        let kind = completion.outcome.kind();
        if let JobOutcome::Failure(cause) = &completion.outcome {
            tracing::warn!(operation="installation_shell_publication_failed", cause=%crate::failure_details::Details(cause),
                "Application is healthy; installation shell access still needs repair");
        } else if kind != TerminalKind::Success {
            tracing::warn!(
                operation = "installation_shell_publication_stopped",
                ?kind,
                "Application is healthy; shell publication did not finish"
            );
        }
        self.completion = Some(terminal::Terminal::observe(completion));
        self.job = None;
        if valid { Ok(()) } else { Err(AuthorityIssue::Identity) }
    }
}

#[cfg(test)]
#[path = "shell/tests.rs"]
mod tests;
