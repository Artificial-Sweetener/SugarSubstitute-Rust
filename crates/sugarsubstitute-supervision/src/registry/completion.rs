// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Consume exact typed jobs; metadata, cancellation and native root exit are not family proof.
use super::*;
use sugarsubstitute_contracts::execution::{JobCompletion, JobOutcome};

impl WindowsProcessRegistry {
    /// Consume at most32 metadata entries through exact registry-held typed jobs.
    /// Unknown notifications remain facts for other owners; they cause no registry
    /// mutation. Early watch metadata is retained until its creation/cleanup owner
    /// returns. An empty activity batch never fabricates a native outcome.
    pub fn consume(&mut self, batch: &CompletionBatch) -> Result<usize, RegistryError> {
        if batch.notifications.len() > 32 {
            return Err(RegistryError::invalid("completion_batch_bound"));
        }
        let mut matched = 0;
        for notification in &batch.notifications {
            for record in self.records.iter_mut().flatten() {
                let identity = (notification.scope_id, notification.job_id);
                if record
                    .creation
                    .as_ref()
                    .is_some_and(|job| (job.scope_id(), job.spec().id) == identity)
                {
                    if let Some(completion) =
                        record.creation.as_mut().and_then(OwnedJob::try_take_completion)
                    {
                        drop(record.creation.take());
                        creation(record, completion);
                    }
                    matched += 1;
                    break;
                }
                if record
                    .cleanup
                    .as_ref()
                    .is_some_and(|job| (job.scope_id(), job.spec().id) == identity)
                {
                    if let Some(completion) =
                        record.cleanup.as_mut().and_then(OwnedJob::try_take_completion)
                    {
                        drop(record.cleanup.take());
                        cleanup(record, completion);
                    }
                    matched += 1;
                    break;
                }
                if record.root_spec == identity {
                    record.root_notice = true;
                    matched += 1;
                    break;
                }
                if record.member_spec == Some(identity) {
                    record.member_notice = true;
                    matched += 1;
                    break;
                }
            }
        }
        for record in self.records.iter_mut().flatten() {
            // Shared native state is acquired only after its finite worker job
            // actually returned; metadata cannot make the UI wait on native FFI.
            if record.creation.is_none() && record.cleanup.is_none() {
                if record.root_notice
                    && let Some(completion) =
                        record.root_watch.as_mut().and_then(OwnedJob::try_take_completion)
                {
                    drop(record.root_watch.take());
                    record.root_notice = false;
                    observe_watch(
                        record,
                        completion.outcome,
                        record.root,
                        true,
                        completion.spec.id,
                    );
                }
                if record.member_notice
                    && let Some(completion) =
                        record.member_watch.as_mut().and_then(OwnedJob::try_take_completion)
                {
                    drop(record.member_watch.take());
                    record.member_spec = None;
                    record.member_notice = false;
                    self.cleanup_watches -= 1;
                    let watched = record.watched.take();
                    observe_watch(record, completion.outcome, watched, false, completion.spec.id);
                }
            }
            if !record.settled
                && record.closed_native
                && record.creation.is_none()
                && record.cleanup.is_none()
                && record.root_watch.is_none()
                && record.member_watch.is_none()
            {
                record.settled = true;
                record.unread[2] = !record.rejected_launch;
            }
        }
        for slot in &mut self.records {
            if slot.as_ref().is_some_and(|record| record.rejected_launch && record.settled) {
                *slot = None;
            }
        }
        Ok(matched)
    }
}
fn creation(record: &mut Record, completion: JobCompletion<(), RegistryError>) {
    let (facts, failure, watch, no_family) = {
        let mut state = lock(&record.shared.native);
        (state.creation.take(), state.failure.take(), state.watch.take(), state.family.is_none())
    };
    record.root_watch = watch;
    if let Some(permit) = lock(&record.shared.permit).take() {
        record.root_watch = Some(permit.cancel_unbound());
    }
    record.closed_native = no_family;
    if let Some(facts) = facts {
        record.root = Some(facts.root);
        record.retained = 1;
        if facts.resumed {
            record.started = true;
            record.unread[0] = true;
        }
    }
    retained_failure(record, failure, completion.outcome, "creation_terminal");
    if record.error.is_some() {
        record.shared.admission.store(CLOSED, Ordering::Release);
    }
}
fn cleanup(record: &mut Record, completion: JobCompletion<(), RegistryError>) {
    let (facts, failure, watch) = {
        let mut state = lock(&record.shared.native);
        (state.cleanup.take(), state.failure.take(), state.watch.take())
    };
    if let Some(watch) = watch {
        record.member_watch = Some(watch);
    }
    if let Some(permit) = lock(&record.shared.permit).take() {
        record.member_watch = Some(permit.cancel_unbound());
        record.watched = None;
    }
    if let Some(facts) = facts {
        record.retained = facts.retained;
        record.root = facts.root.or(record.root);
        record.next = facts.next;
        record.closed_native = facts.closed;
        record.pending_exit = None;
        if let Some(watched) = facts.watched {
            record.watched = Some(watched);
        }
    }
    retained_failure(record, failure, completion.outcome, "cleanup_terminal");
}
fn retained_failure(
    record: &mut Record,
    failure: Option<RegistryError>,
    outcome: JobOutcome<(), RegistryError>,
    stage: &'static str,
) {
    if let Some(failure) = failure {
        record.fail(failure);
        if let JobOutcome::Panicked(cause) = outcome {
            record.fail(RegistryError::owner_panic(stage, cause));
        }
    } else {
        terminal_failure(record, outcome, stage);
    }
}
fn terminal_failure(
    record: &mut Record,
    outcome: JobOutcome<(), RegistryError>,
    stage: &'static str,
) {
    match outcome {
        JobOutcome::Success(()) => {}
        JobOutcome::Failure(error) => record.fail(error),
        JobOutcome::Panicked(cause) => record.fail(RegistryError::owner_panic(stage, cause)),
        JobOutcome::Cancelled | JobOutcome::TimedOut | JobOutcome::Coalesced { .. } => {
            record.fail(RegistryError::invalid(stage))
        }
    }
}
pub(super) fn observe_watch(
    record: &mut Record,
    outcome: JobOutcome<ProcessExit, NativeObservationError>,
    expected: Option<Identity>,
    root: bool,
    job: JobId,
) {
    if root && expected.is_some() && !record.rejected_launch {
        let retirement = match &outcome {
            JobOutcome::Cancelled => Some(RootObservationOutcome::Cancelled),
            JobOutcome::TimedOut => Some(RootObservationOutcome::TimedOut),
            _ => None,
        };
        if let Some(retirement) = retirement {
            record.root_retirement = Some((job, retirement));
            record.unread[4] = true;
        }
    }
    match outcome {
        JobOutcome::Success(exit) => {
            let identity = Identity { pid: exit.process_id, birth: exit.creation_time_100ns };
            if exit.correlation.get() != record.ticket.process.id.get()
                || Some(identity) != expected
            {
                record.fail(RegistryError::invalid("watch_exit_identity"));
                return;
            }
            if root || record.root == Some(identity) {
                record.root_exit = Some(exit);
                record.unread[1] = true;
            }
            if !root {
                record.pending_exit = Some(identity);
            }
            if record.next.is_some_and(|(_, next)| next == identity) {
                record.next = None;
            }
        }
        JobOutcome::Failure(cause) => {
            record.fail(RegistryError::native_observation("process_observation", cause))
        }
        JobOutcome::Panicked(cause) => {
            record.fail(RegistryError::owner_panic("process_observation", cause))
        }
        JobOutcome::Cancelled if expected.is_none() || record.rejected_launch => {}
        JobOutcome::Cancelled | JobOutcome::TimedOut | JobOutcome::Coalesced { .. } => {
            record.fail(RegistryError::invalid("process_observation_not_exit"))
        }
    }
}
