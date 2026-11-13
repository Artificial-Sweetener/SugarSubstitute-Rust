// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Use real admitted delivery to preserve notices across independent protocol failure.
use super::*;
use crate::completion_wake::Wake;
use std::time::Duration;
use sugarsubstitute_contracts::execution::{Clock, ScopeId, ScopeSpec};
fn child() -> Result<(Child, Arc<Wake>)> {
    let mut child = Child::new(
        ChildOptions { role: Role::Corpus, section: None, deadline: None },
        crate::diagnostics::DiagnosticHealth::fixture(),
    );
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(child.clock.clone(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let spec = child.spec(40960)?;
    child.writes[0] = Some(client.submit_blocking(&scope, spec, |context| {
        let _claim = context.memory().claim(32768).map_err(io::Error::other)?;
        Ok::<_, io::Error>(Written { bytes: 7, writes: 1 })
    })?);
    child.scope = Some(scope);
    child.client = Some(client);
    child.owner = Some(owner);
    Ok((child, wake))
}
/// Deliver completions until the child holds the writer's terminal notice.
fn capture(child: &mut Child, wake: &Wake) -> Result<()> {
    wake.until(|| {
        let captured = (|| -> Result<bool> {
            let batch = child
                .client
                .as_ref()
                .ok_or("Controlled client missing")?
                .drain_completions(NonZeroUsize::MIN)?;
            child.capture(&batch)?;
            Ok(child.write_notices[0].is_some())
        })();
        match captured {
            Ok(false) => None,
            Ok(true) => Some(Ok(())),
            Err(cause) => Some(Err(cause)),
        }
    })
}
fn finish(mut child: Child, _: &Wake) -> Result<()> {
    let deadline = child.clock.now().checked_add(Duration::from_secs(5))?;
    let owner = child.owner.as_mut().ok_or("Controlled owner missing")?;
    owner.request_shutdown(deadline);
    // The owner joins its private workers on test teardown. Worker exit is not a
    // new terminal mailbox publication and need not supply another native wake.
    drop(child);
    Ok(())
}
#[test]
fn failed_retirement_does_not_readmit_on_unchanged_closing_callbacks() -> Result<()> {
    let mut child = Child::new(
        ChildOptions { role: Role::Forward, section: None, deadline: None },
        crate::diagnostics::DiagnosticHealth::fixture(),
    );
    child.closing = true;
    child.retirement_requested = true;
    assert!(!child.take_retirement_request(false));
    assert!(child.take_retirement_request(true));
    // A failed native return supplies no new cleanup demand.
    for _ in 0..64 {
        assert!(!child.take_retirement_request(true));
    }
    child.deadline = Some(Moment::from_nanos(0));
    child.observe_deadline()?;
    assert!(child.take_retirement_request(true));
    child.observe_deadline()?;
    assert!(!child.take_retirement_request(true));
    assert_eq!(child.next_deadline(), None);
    Ok(())
}
#[test]
fn descendant_jobs_use_containment_admission_and_preserve_original_deadline() -> Result<()> {
    let mut child = Child::new(
        ChildOptions { role: Role::Forward, section: None, deadline: None },
        crate::diagnostics::DiagnosticHealth::fixture(),
    );
    child.deadline = Some(Moment::from_nanos(123));
    let creation = child.descendant_spec(4096)?;
    let retirement = child.descendant_spec(8192)?;
    for spec in [creation, retirement] {
        assert_eq!(spec.operation, Operation::NativeLifecycle);
        assert_eq!(spec.class, JobClass::InfrastructureLifecycle);
        assert_eq!(spec.deadline, child.deadline);
    }
    assert!(creation.id < retirement.id);
    assert_eq!(child.spec(40960)?.operation, Operation::Infrastructure);
    Ok(())
}
#[test]
fn completed_descendant_leaf_does_not_close_writers_on_its_finished_write_deadline() -> Result<()> {
    let mut child = Child::new(
        ChildOptions { role: Role::Leaf, section: None, deadline: None },
        crate::diagnostics::DiagnosticHealth::fixture(),
    );
    child.deadline = Some(Moment::from_nanos(0));
    child.deadline_wall = Some(sugarsubstitute_execution::qualification_time::anchor());
    child.done_queued = true;
    child.observe_deadline()?;
    assert!(!child.deadline_observed && !child.closing);
    assert_eq!(child.next_deadline(), None);
    Ok(())
}
#[derive(Default)]
struct ControlledClock(std::sync::atomic::AtomicU64);
impl Clock for ControlledClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(std::sync::atomic::Ordering::Acquire))
    }
    fn sleep_until(&self, _: Moment) -> sugarsubstitute_contracts::execution::WaitFuture {
        Box::pin(std::future::pending())
    }
}
struct Sink {
    clock: Arc<ControlledClock>,
    calls: Arc<std::sync::atomic::AtomicU64>,
    flushes: Arc<std::sync::atomic::AtomicU64>,
}
impl io::Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        self.clock.0.store(100, std::sync::atomic::Ordering::Release);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.flushes.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        Ok(())
    }
}
fn deadline_write(role: Role) -> Result<()> {
    use std::sync::atomic::{AtomicU64, Ordering};
    let clock = Arc::new(ControlledClock::default());
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let calls = Arc::new(AtomicU64::new(0));
    let flushes = Arc::new(AtomicU64::new(0));
    let mut sink = Sink { clock, calls: calls.clone(), flushes: flushes.clone() };
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(100)),
            reservation_bytes: 40960,
            replacement: None,
        },
        move |context| {
            super::super::writers::write(&context, role, OutputStream::Stdout, &mut sink)
        },
    )?;
    assert_eq!(wake.completion(&client, &mut job)?.outcome.kind(), TerminalKind::TimedOut);
    assert_eq!(calls.load(Ordering::Acquire), 1);
    assert_eq!(flushes.load(Ordering::Acquire), 0);
    drop(job);
    drop(owner);
    Ok(())
}
#[test]
fn original_deadline_prevents_later_source_write_after_first_actual_write() -> Result<()> {
    deadline_write(Role::Corpus)
}
#[test]
fn original_deadline_prevents_flush_after_last_actual_source_write() -> Result<()> {
    deadline_write(Role::Leaf)
}
#[test]
fn protocol_failure_after_drained_writer_notice_does_not_lose_actual_delivery() -> Result<()> {
    let (mut child, wake) = child()?;
    capture(&mut child, &wake)?;
    assert!(super::super::channel::decode(Role::Corpus, b"bad").is_err());
    child.closing = true;
    child.consume_write(0)?;
    assert!(child.writes[0].is_none());
    assert_eq!(child.written[0], Some(Written { bytes: 7, writes: 1 }));
    finish(child, &wake)
}
#[test]
fn validation_failure_after_actual_transfer_clears_owned_wait_marker() -> Result<()> {
    let (mut child, wake) = child()?;
    capture(&mut child, &wake)?;
    child.write_notices[0] = Some(TerminalKind::Failure);
    assert!(child.consume_write(0).is_err());
    assert!(child.writes[0].is_none());
    assert!(child.write_notices[0].is_none());
    assert!(child.writes_settled());
    finish(child, &wake)
}
