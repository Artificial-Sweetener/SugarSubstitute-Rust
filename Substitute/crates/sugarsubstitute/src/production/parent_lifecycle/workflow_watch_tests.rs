// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The watch's reading of finished jobs: changes, quiet renewals and reconnect backoff.
use super::*;

fn at(seconds: u64) -> Moment {
    Moment::from_nanos(seconds * 1_000_000_000)
}

#[test]
fn a_change_is_adopted_and_a_quiet_window_keeps_what_was_known() {
    let mut watch = WorkflowWatch::default();
    assert_eq!(watch.queue(), WorkflowQueue::Unobserved);
    watch.settle(JobOutcome::Success(QueueObservation::Changed(QueueActivity::Busy)), Some(at(1)));
    assert_eq!(watch.queue(), WorkflowQueue::Busy);
    watch.settle(JobOutcome::Success(QueueObservation::Quiet), Some(at(2)));
    assert_eq!(watch.queue(), WorkflowQueue::Busy);
    assert_eq!(watch.deadline(), None, "a healthy watch reconnects at once");
    watch.settle(JobOutcome::Success(QueueObservation::Changed(QueueActivity::Idle)), Some(at(3)));
    assert_eq!(watch.queue(), WorkflowQueue::Idle);
}

#[test]
fn a_short_outage_keeps_the_last_queue_and_reconnects_on_a_doubling_backoff() {
    let mut watch = WorkflowWatch::default();
    watch.settle(JobOutcome::Success(QueueObservation::Changed(QueueActivity::Busy)), Some(at(0)));
    watch.settle(JobOutcome::Success(QueueObservation::Closed), Some(at(10)));
    assert_eq!(watch.queue(), WorkflowQueue::Busy, "a reconnect learns the queue again");
    assert_eq!(watch.deadline(), Some(at(11)));
    let refused =
        || JobOutcome::Failure(std::io::Error::from(std::io::ErrorKind::ConnectionRefused));
    watch.settle(refused(), Some(at(11)));
    assert_eq!(watch.deadline(), Some(at(13)));
    watch.settle(refused(), Some(at(13)));
    assert_eq!(watch.deadline(), Some(at(17)));
    assert_eq!(watch.queue(), WorkflowQueue::Busy, "three failures are still an outage");
    watch.settle(JobOutcome::Success(QueueObservation::Changed(QueueActivity::Idle)), Some(at(17)));
    assert_eq!(watch.queue(), WorkflowQueue::Idle, "the reconnect's first status is the queue");
    watch.settle(JobOutcome::Cancelled, Some(at(20)));
    assert_eq!(watch.deadline(), Some(at(21)), "a status resets the backoff");
    assert_eq!(watch.queue(), WorkflowQueue::Idle);
}

#[test]
fn repeated_failed_connections_lose_the_backend_and_never_assume_an_idle_queue() {
    let mut watch = WorkflowWatch::default();
    watch.settle(JobOutcome::Success(QueueObservation::Changed(QueueActivity::Busy)), Some(at(0)));
    let mut expected = 0;
    for (attempt, backoff) in [1, 2, 4, 8, 16, 30, 30].into_iter().enumerate() {
        watch.settle(
            JobOutcome::Failure(std::io::Error::from(std::io::ErrorKind::ConnectionRefused)),
            Some(at(expected)),
        );
        expected += backoff;
        assert_eq!(watch.deadline(), Some(at(expected)));
        let lost = attempt + 1 >= LOST_AFTER as usize;
        let shown = if lost { WorkflowQueue::Unobserved } else { WorkflowQueue::Busy };
        assert_eq!(watch.queue(), shown, "after {} failures", attempt + 1);
    }
    watch.settle(JobOutcome::TimedOut, None);
    assert_eq!(watch.deadline(), None, "without a clock the next turn retries");
    assert_eq!(watch.queue(), WorkflowQueue::Unobserved);
    watch.settle(JobOutcome::Success(QueueObservation::Quiet), Some(at(500)));
    assert_eq!(watch.queue(), WorkflowQueue::Unobserved, "a quiet window states no queue");
    watch
        .settle(JobOutcome::Success(QueueObservation::Changed(QueueActivity::Idle)), Some(at(501)));
    assert_eq!(watch.queue(), WorkflowQueue::Idle);
}

#[test]
fn a_healthy_quiet_window_resets_the_outage_count() {
    let mut watch = WorkflowWatch::default();
    watch.settle(JobOutcome::Success(QueueObservation::Changed(QueueActivity::Busy)), Some(at(0)));
    for round in 0..3 {
        for _ in 0..LOST_AFTER - 1 {
            watch.settle(JobOutcome::Success(QueueObservation::Closed), Some(at(round)));
        }
        watch.settle(JobOutcome::Success(QueueObservation::Quiet), Some(at(round)));
    }
    assert_eq!(watch.queue(), WorkflowQueue::Busy, "outages separated by healthy windows");
}

#[test]
fn stopping_forgets_the_queue_and_any_reconnect() {
    let mut watch = WorkflowWatch::default();
    watch.settle(JobOutcome::Success(QueueObservation::Closed), Some(at(5)));
    watch.stop();
    assert_eq!(watch.queue(), WorkflowQueue::Unobserved);
    assert_eq!(watch.deadline(), None);
}

/// The watch's connection job is admitted: an ordinary child scope of the backend's native
/// scope, which refuses tasks, and the lifecycle I/O class a readiness watch takes. Either
/// refusal failed every connection before it was made, so the backend always read as lost.
#[test]
fn a_watch_is_admitted_beside_the_backends_native_scope() -> Result<(), Box<dyn std::error::Error>>
{
    use crate::{job_sequence::JobSequence, scope_sequence::ScopeSequence};
    use std::sync::Arc;
    use sugarsubstitute_contracts::{
        configuration::LoopbackOrigin,
        coordinator_entry::EntrySetupBudget,
        execution::{Clock, CompletionWake, ExecutionError, WaitFuture},
    };
    struct Frozen;
    impl Clock for Frozen {
        fn now(&self) -> Moment {
            Moment::from_nanos(0)
        }
        fn sleep_until(&self, _: Moment) -> WaitFuture {
            Box::pin(std::future::pending())
        }
    }
    struct Wake;
    impl CompletionWake for Wake {
        fn wake(&self) -> Result<(), ExecutionError> {
            Ok(())
        }
    }
    let execution =
        sugarsubstitute_execution::ExecutionOwner::new(Arc::new(Frozen), Arc::new(Wake))?;
    let client = execution.client();
    let scopes = ScopeSequence::new();
    let native =
        client.create_native_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, None)?;
    let cleanup =
        client.create_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, None)?;
    let jobs = JobSequence::new();
    let mut authority = crate::production::coordinator_authority::CoordinatorAuthority::new(
        EntrySetupBudget::new(Moment::from_nanos(0))?,
        None,
    )?;
    let resources = BackendResources {
        client: &client,
        native: &native,
        cleanup: &cleanup,
        jobs: &jobs,
        scopes: &scopes,
        authority: &mut authority,
        capture: None,
    };
    let mut watch = WorkflowWatch::default();
    watch.advance(Some(LoopbackOrigin::parse("http://127.0.0.1:9/", 9)?), &resources);
    assert!(watch.job.is_some(), "the connection job was admitted");
    assert_eq!(watch.failures, 0);
    assert!(watch.deadline().is_none(), "no reconnect is pending");
    watch.stop();
    Ok(())
}
