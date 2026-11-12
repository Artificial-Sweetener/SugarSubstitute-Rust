// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Follow the granted backend's ComfyUI queue so the desktop knows whether a workflow runs.
//!
//! One async job at a time holds ComfyUI's WebSocket for a bounded window and ends on the first
//! queue change; the next job starts with what the last one learned. A connection that fails or
//! closes makes the queue unobserved until a reconnect, after a backoff that doubles up to
//! [`MAX_BACKOFF`] and resets once a status arrives. A new origin, or none, drops the old job
//! and starts over. Nothing here can fail the parent: a watch that cannot run reads as
//! unobserved, and the desktop decides what that means for a waiting restart.
use std::time::Duration;
use sugarsubstitute_comfy::queue_watch::{self, QueueActivity, QueueObservation};
use sugarsubstitute_contracts::{
    configuration::LoopbackOrigin,
    execution::{ExecutionPort, JobClass, JobOutcome, JobSpec, Moment, Operation, OwnedJob},
};
use sugarsubstitute_supervision::lifecycle_protocol::WorkflowQueue;

use crate::production::coordinator_backend::BackendResources;

/// How long one connection is held before it is renewed.
const WINDOW: Duration = Duration::from_secs(300);
/// Allowance past the window for the opening handshake and the close.
const MARGIN: Duration = Duration::from_secs(15);
/// First wait before reconnecting after a failure.
const FIRST_BACKOFF: Duration = Duration::from_secs(1);
/// Longest wait before reconnecting.
const MAX_BACKOFF: Duration = Duration::from_secs(30);

/// The queue as last observed, and the job observing it.
#[derive(Default)]
pub(super) struct WorkflowWatch {
    queue: Option<QueueActivity>,
    origin: Option<LoopbackOrigin>,
    job: Option<OwnedJob<QueueObservation, std::io::Error>>,
    retry_at: Option<Moment>,
    backoff: Option<Duration>,
}
impl WorkflowWatch {
    /// What the desktop is told.
    pub(super) fn queue(&self) -> WorkflowQueue {
        match self.queue {
            None => WorkflowQueue::Unobserved,
            Some(QueueActivity::Idle) => WorkflowQueue::Idle,
            Some(QueueActivity::Busy) => WorkflowQueue::Busy,
        }
    }

    /// When a reconnect is due, so the composing wait wakes for it.
    pub(super) fn deadline(&self) -> Option<Moment> {
        self.retry_at
    }

    /// Drop the job; the queue reads as unobserved until a later [`Self::advance`] with an
    /// origin.
    pub(super) fn stop(&mut self) {
        *self = Self::default();
    }

    /// Follow `origin`, the granted backend's ready origin, or stop following when `None`.
    /// Consumes the job's completion and starts the next job when one is due.
    pub(super) fn advance(
        &mut self,
        origin: Option<LoopbackOrigin>,
        resources: &BackendResources<'_>,
    ) {
        if origin != self.origin {
            *self = Self { origin, ..Self::default() };
        }
        let Some(origin) = self.origin else { return };
        let now = resources.client.now().ok();
        if let Some(job) = &mut self.job {
            let Some(completion) = job.try_take_completion() else { return };
            self.job = None;
            self.settle(completion.outcome, now);
        }
        let Some(now) = now else { return self.lost(None, "The execution clock failed") };
        if self.retry_at.is_some_and(|retry_at| now < retry_at) {
            return;
        }
        self.retry_at = None;
        if let Err(cause) = self.submit(origin, now, resources) {
            self.lost(Some(now), &cause.to_string());
        }
    }

    /// Adopt one finished watch.
    fn settle(
        &mut self,
        outcome: JobOutcome<QueueObservation, std::io::Error>,
        now: Option<Moment>,
    ) {
        match outcome {
            JobOutcome::Success(QueueObservation::Changed(activity)) => {
                self.queue = Some(activity);
                self.backoff = None;
            }
            JobOutcome::Success(QueueObservation::Quiet) => {}
            JobOutcome::Success(QueueObservation::Closed) => {
                self.lost(now, "ComfyUI closed its queue connection");
            }
            JobOutcome::Failure(cause) => self.lost(now, &cause.to_string()),
            other => self.lost(now, &format!("{:?}", other.kind())),
        }
    }

    fn submit(
        &mut self,
        origin: LoopbackOrigin,
        now: Moment,
        resources: &BackendResources<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let target = queue_watch::target(origin)?;
        let until = now.checked_add(WINDOW)?;
        let known = self.queue;
        let spec = JobSpec {
            id: resources.jobs.allocate()?,
            operation: Operation::Readiness,
            class: JobClass::IoOrdinary,
            deadline: Some(until.checked_add(MARGIN)?),
            reservation_bytes: queue_watch::RESERVATION_BYTES,
            replacement: None,
        };
        self.job = Some(resources.client.submit_async(
            resources.native,
            spec,
            move |context| async move {
                queue_watch::next_change(&target, &context, until, known).await
            },
        )?);
        Ok(())
    }

    /// The queue is unknown until a reconnect after the next backoff. Without a clock reading
    /// the reconnect waits for the next turn that has one.
    fn lost(&mut self, now: Option<Moment>, cause: &str) {
        let backoff = self.backoff.map_or(FIRST_BACKOFF, |last| (last * 2).min(MAX_BACKOFF));
        tracing::warn!(
            operation = "workflow_queue_unobserved",
            cause,
            retry_seconds = backoff.as_secs(),
            "The backend's workflow queue is not observed; reconnecting"
        );
        self.queue = None;
        self.backoff = Some(backoff);
        self.retry_at = now.and_then(|now| now.checked_add(backoff).ok());
    }
}

#[cfg(test)]
#[path = "workflow_watch_tests.rs"]
mod tests;
