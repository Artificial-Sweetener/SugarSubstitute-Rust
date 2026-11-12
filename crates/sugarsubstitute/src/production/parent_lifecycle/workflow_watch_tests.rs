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
fn losing_the_connection_unobserves_the_queue_and_backs_off_until_a_status_arrives() {
    let mut watch = WorkflowWatch::default();
    watch.settle(JobOutcome::Success(QueueObservation::Changed(QueueActivity::Busy)), Some(at(0)));
    watch.settle(JobOutcome::Success(QueueObservation::Closed), Some(at(10)));
    assert_eq!(watch.queue(), WorkflowQueue::Unobserved, "a busy queue is never assumed idle");
    assert_eq!(watch.deadline(), Some(at(11)));
    let mut expected = 11;
    for doubled in [2, 4, 8, 16, 30, 30] {
        watch.settle(
            JobOutcome::Failure(std::io::Error::from(std::io::ErrorKind::ConnectionRefused)),
            Some(at(expected)),
        );
        expected += doubled;
        assert_eq!(watch.deadline(), Some(at(expected)));
    }
    watch.settle(JobOutcome::TimedOut, None);
    assert_eq!(watch.deadline(), None, "without a clock the next turn retries");
    watch.settle(JobOutcome::Success(QueueObservation::Changed(QueueActivity::Idle)), Some(at(99)));
    assert_eq!(watch.queue(), WorkflowQueue::Idle);
    watch.settle(JobOutcome::Cancelled, Some(at(100)));
    assert_eq!(watch.deadline(), Some(at(101)), "a status resets the backoff");
}

#[test]
fn stopping_forgets_the_queue_and_any_reconnect() {
    let mut watch = WorkflowWatch::default();
    watch.settle(JobOutcome::Success(QueueObservation::Closed), Some(at(5)));
    watch.stop();
    assert_eq!(watch.queue(), WorkflowQueue::Unobserved);
    assert_eq!(watch.deadline(), None);
}
