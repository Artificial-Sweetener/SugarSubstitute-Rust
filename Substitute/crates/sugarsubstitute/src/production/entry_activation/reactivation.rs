// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coalesce repeated launch requests before acquiring another browser profile.
//!
//! A repeated launch for a profile that already has a desktop either brings that window
//! forward or tells the launching process the window is not responding. Responsiveness comes
//! from the watchdog's latched suspicion or from an owned blocking probe of the desktop's
//! windows, never from elapsed time.
use super::*;
use sugarsubstitute_contracts::execution::{ExecutionPort, Scope};
use sugarsubstitute_desktop::{ProcessError, ProcessResponsiveness};
use sugarsubstitute_execution::{ExecutionClient, windows_lifecycle_pipe::VerifiedLifecyclePeer};

/// A repeated entry either joins the existing window or waits for its presentation.
pub(super) enum Reuse {
    New,
    Waiting,
    /// The request was answered: its window came forward, or it is not responding.
    Replied,
    Expired,
}

/// The probe implementation; production asks the desktop adapter, tests substitute an answer.
pub(in crate::production) type ProbeProcess =
    fn(u32, u64) -> std::result::Result<ProcessResponsiveness, ProcessError>;

/// One owned responsiveness probe of a verified process: a repeated launch probes the existing
/// desktop, and a forwarded launch probes a running copy that has not answered.
#[derive(Default)]
pub(in crate::production) struct ResponsivenessProbe {
    /// The verified identity the job or answer belongs to.
    target: Option<(u32, u64)>,
    job: Option<OwnedJob<ProcessResponsiveness, ProcessError>>,
    answer: Option<ProcessResponsiveness>,
}
impl ResponsivenessProbe {
    /// Return the probe's answer for `target` once its blocking job has completed, submitting
    /// it first. The job's completion wakes the coordinator, so no turn waits on the desktop.
    /// An answer or a job in flight for another identity is never reported for `target`: the
    /// stale job is left to finish under its own bound and its answer is discarded.
    pub(in crate::production) fn observe(
        &mut self,
        target: (u32, u64),
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &crate::job_sequence::JobSequence,
        probe: ProbeProcess,
    ) -> Result<Option<ProcessResponsiveness>> {
        if self.target != Some(target) {
            self.answer = None;
        }
        if let Some(answer) = self.answer {
            return Ok(Some(answer));
        }
        if let Some(job) = &mut self.job {
            let Some(completion) = job.try_take_completion() else { return Ok(None) };
            self.job = None;
            let answer = match completion.outcome {
                JobOutcome::Success(answer) => answer,
                JobOutcome::Failure(cause) => return Err(Box::new(cause)),
                _ => return Err("Desktop responsiveness probe did not complete".into()),
            };
            if self.target == Some(target) {
                self.answer = Some(answer);
                return Ok(Some(answer));
            }
        }
        let (pid, birth) = target;
        self.job = Some(client.submit_blocking(
            scope,
            JobSpec {
                id: jobs.allocate()?,
                operation: Operation::NativeLifecycle,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(client.now()?.checked_add(PROBE_JOB_BOUND)?),
                reservation_bytes: 4096,
                replacement: None,
            },
            move |_| probe(pid, birth),
        )?);
        self.target = Some(target);
        Ok(None)
    }
    /// Forget the answer so the next observation probes again.
    pub(in crate::production) fn reset(&mut self) {
        self.answer = None;
    }
}
/// The probe waits at most one second per window; this bounds the job, not the decision.
const PROBE_JOB_BOUND: std::time::Duration = std::time::Duration::from_secs(30);

/// What a repeated launch does with the existing desktop's current state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Decision {
    Probe,
    Restore,
    Unresponsive,
    Wait,
}
/// Watchdog suspicion answers at once; otherwise only a completed probe decides.
fn decide(suspected: bool, presented: bool, probed: Option<ProcessResponsiveness>) -> Decision {
    if suspected {
        return Decision::Unresponsive;
    }
    if !presented {
        return Decision::Wait;
    }
    match probed {
        None => Decision::Probe,
        Some(ProcessResponsiveness::Responsive) => Decision::Restore,
        Some(ProcessResponsiveness::Unresponsive) => Decision::Unresponsive,
        Some(ProcessResponsiveness::Exited) => Decision::Wait,
    }
}

/// Keep closing profiles on the existing successor-retirement path.
pub(super) fn reuse(
    records: &[Option<RetainedRecord>],
    pending: &mut PendingProjection,
    resources: &mut EntryResources<'_>,
    probe: ProbeProcess,
) -> Result<Reuse> {
    if pending.restart {
        return Ok(Reuse::New);
    }
    let Some(JobOutcome::Success(value)) = pending.completion.as_ref().map(|done| &done.outcome)
    else {
        return Ok(Reuse::New);
    };
    let requested = &value.requested_profile;
    let (connection, request, until) = (value.entry_connection, value.entry_request, value.until);
    if resources.client.now()? >= until
        && records.iter().flatten().any(|held| {
            held.record.profile_projection().is_some_and(|old| old.requested_profile == *requested)
        })
    {
        tracing::info!(
            operation = "entry_repeat_expired",
            "Retired an expired repeated launch without changing the existing desktop"
        );
        return Ok(Reuse::Expired);
    }
    let existing = records.iter().flatten().map(|held| &held.record).find(|record| {
        !record.closed
            && record.failed.is_none()
            && record.pending_desktop_failure.is_none()
            && record.parent.as_ref().is_none_or(|parent| parent.normal_close_request().is_none())
            && record.profile_projection().is_some_and(|old| old.requested_profile == *requested)
    });
    let Some(existing) = existing else { return Ok(Reuse::New) };
    let parent = existing.parent.as_ref();
    let suspected = parent.is_some_and(|parent| parent.ui_suspected());
    let peer = parent.and_then(|parent| parent.activation_peer());
    let probed = match peer {
        Some(peer) if !suspected => pending.probe.observe(
            (peer.process_id(), peer.creation_time_100ns()),
            resources.client,
            resources.finite,
            resources.jobs,
            probe,
        )?,
        _ => None,
    };
    let client = resources.client;
    let control = &resources.intake.server().control;
    let reply = |outcome: EntryOutcome| -> Result<()> {
        let deadline = client.now()?.checked_add(std::time::Duration::from_secs(2))?.min(until);
        control.reply(connection, request, EntryOperationId::new(1)?, deadline, outcome)?;
        Ok(())
    };
    match decide(suspected, peer.is_some(), probed) {
        Decision::Probe => Ok(Reuse::Waiting),
        Decision::Wait => {
            pending.probe.reset();
            Ok(Reuse::Waiting)
        }
        Decision::Unresponsive => {
            tracing::warn!(
                operation = "entry_repeat_unresponsive",
                suspected,
                "The existing desktop is not responding; the repeated launch decides what to do"
            );
            reply(EntryOutcome::Unresponsive)?;
            Ok(Reuse::Replied)
        }
        Decision::Restore => restore_and_reply(
            peer,
            |peer| {
                sugarsubstitute_desktop::activate_desktop_process(
                    peer.process_id(),
                    peer.creation_time_100ns(),
                )
                .map_err(|cause| Box::new(cause) as Box<dyn Error>)
            },
            || reply(EntryOutcome::Accepted(existing.run)),
        ),
    }
}

/// Do not acknowledge an invisible owner before its native window has been restored.
fn restore_and_reply(
    peer: Option<VerifiedLifecyclePeer>,
    restore: impl FnOnce(VerifiedLifecyclePeer) -> Result<bool>,
    reply: impl FnOnce() -> Result<()>,
) -> Result<Reuse> {
    let Some(peer) = peer else { return Ok(Reuse::Waiting) };
    if !restore(peer)? {
        return Ok(Reuse::Waiting);
    }
    reply()?;
    Ok(Reuse::Replied)
}

#[cfg(test)]
#[path = "reactivation_tests.rs"]
mod tests;
