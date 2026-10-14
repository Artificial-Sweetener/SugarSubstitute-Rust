// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Close the real finite output owner; fake only returned wire bytes and worker boundaries.
use super::*;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use sugarsubstitute_supervision::lifecycle_protocol::ReplyOutcome;

#[derive(Default)]
struct Gate {
    open: Mutex<bool>,
    changed: Condvar,
}
impl Gate {
    fn open(&self) {
        *self.open.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = true;
        self.changed.notify_all();
    }
    fn wait(&self) -> TestResult {
        let mut open = self.open.lock().map_err(|_| "Boundary poisoned")?;
        while !*open {
            let next = self
                .changed
                .wait_timeout(open, Duration::from_secs(10))
                .map_err(|_| "Boundary poisoned")?;
            open = next.0;
            if next.1.timed_out() && !*open {
                return Err("External worker boundary did not advance".into());
            }
        }
        Ok(())
    }
}
struct Release(Arc<Gate>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.open();
    }
}
fn bytes(header: WireHeader, reply: ReplyOutcome<'_>) -> TestResult<Vec<u8>> {
    let mut bytes = [0; 512];
    let n = FrameCodec::encode(
        &WireMessage { header, payload: WirePayload::Reply(reply) },
        &mut bytes,
    )?;
    Ok(bytes[..n].to_vec())
}

#[test]
fn closed_prepared_pull_preserves_original_until_exact_withdrawal() -> TestResult {
    let rig = Rig::new()?;
    let mut output = output(&rig)?;
    let h = header(binding()?, 2)?;
    let until = Moment::from_nanos(100);
    output.prepare_pull(h, until)?;
    output.close();
    assert_eq!(output.retiring_pull(), Some((h, until, false)));
    assert!(matches!(output.prepare_pull(header(binding()?, 3)?, until), Err(OutputError::Order)));
    assert!(matches!(
        output.finish_retiring_pull(header(binding()?, 3)?),
        Err(OutputError::Identity)
    ));
    assert_eq!(output.retiring_pull(), Some((h, until, false)));
    output.finish_retiring_pull(h)?;
    assert!(output.retiring_pull().is_none());
    assert!(output.acknowledged.is_none());
    assert_eq!(rig.scope.snapshot().reserved_bytes, SOURCE_BYTES);
    Ok(())
}

#[test]
fn closed_admitted_pull_validates_original_idle_without_job_or_ack() -> TestResult {
    let rig = Rig::new()?;
    let mut output = output(&rig)?;
    let b = binding()?;
    let h = header(b, 2)?;
    output.prepare_pull(h, Moment::from_nanos(100))?;
    output.admitted_pull(h)?;
    let reply = bytes(h, ReplyOutcome::OutputIdle(b.lease))?;
    assert!(matches!(output.validate_retiring_reply(&reply), Err(OutputError::Order)));
    output.close();
    assert_eq!(output.validate_retiring_reply(&reply)?, h);
    let mut foreign = b.lease;
    foreign.lease = LeaseId::new(2)?;
    for reply in [
        bytes(header(b, 3)?, ReplyOutcome::OutputIdle(b.lease))?,
        bytes(h, ReplyOutcome::OutputIdle(foreign))?,
    ] {
        assert!(matches!(output.validate_retiring_reply(&reply), Err(OutputError::Identity)));
    }
    assert_eq!(output.retiring_pull(), Some((h, Moment::from_nanos(100), true)));
    assert!(!output.worker_pending());
    assert!(output.acknowledged.is_none());
    Ok(())
}

#[test]
fn closed_delivery_requires_exact_next_id_and_never_publishes_or_acks() -> TestResult {
    let rig = Rig::new()?;
    let mut output = output(&rig)?;
    let b = binding()?;
    let h = header(b, 2)?;
    output.acknowledged = NonZeroU64::new(4);
    output.prepare_pull(h, Moment::from_nanos(100))?;
    output.admitted_pull(h)?;
    output.close();
    for id in [4, 6, 5] {
        let wire = bytes(
            h,
            ReplyOutcome::OutputDelivery(OutputDelivery {
                lease: b.lease,
                delivery_id: NonZeroU64::new(id).ok_or("Zero delivery")?,
                stream: OutputStream::Stdout,
                sequence: StreamSequence::new(1)?,
                gap: None,
                end: false,
                bytes: b"unpublished",
            }),
        )?;
        let result = output.validate_retiring_reply(&wire);
        if id == 5 {
            assert_eq!(result?, h);
        } else {
            assert!(matches!(result, Err(OutputError::Identity)));
        }
    }
    assert_eq!(output.acknowledged.map(NonZeroU64::get), Some(4));
    assert!(
        output
            .source
            .state
            .lock()
            .map_err(|_| "Source poisoned")?
            .transcript
            .snapshot_tail(1)?
            .records()
            .next()
            .is_none()
    );
    assert!(!output.worker_pending());
    Ok(())
}

fn worker_boundary(failure: bool) -> TestResult {
    let mut rig = Rig::new()?;
    let mut output = output(&rig)?;
    let h = header(binding()?, 2)?;
    output.prepare_pull(h, Moment::from_nanos(100))?;
    output.admitted_pull(h)?;
    let entered = Arc::new(Gate::default());
    let returned = Arc::new(Gate::default());
    let release = Release(returned.clone());
    let worker_entered = entered.clone();
    let mut input = Some((output.source.clone(), returned.clone()));
    // The external parser boundary is controlled; scope, source, cancellation, notices,
    // actual finite completion and owner consumption are the production implementations.
    output.job = Some(rig.client.submit_blocking_owned_input(
        &rig.scope,
        spec(JobId::new(2)?, Moment::from_nanos(100), TRANSFER_BYTES),
        &mut input,
        move |(source, returned), context| {
            source.check(&context)?;
            worker_entered.open();
            returned.wait().map_err(|_| OutputError::Poisoned)?;
            if failure {
                return Err(OutputError::Identity);
            }
            worker::check(&context)?;
            Ok(worker::Parsed { delivery: None, change: None, snapshot: None })
        },
    )?);
    entered.wait()?;
    output.close();
    assert!(output.worker_pending());
    assert!(output.take_applied()?.is_none());
    assert!(matches!(output.take_unparsed_frame(), Err(OutputError::Order)));
    assert_eq!(output.retiring_pull(), Some((h, Moment::from_nanos(100), true)));
    assert!(rig.scope.snapshot().reserved_bytes >= SOURCE_BYTES);
    drop(release);
    rig.notice(&mut output)?;
    let cause = output.take_applied().err().ok_or("Closed terminal not observed")?;
    assert!(!output.worker_pending());
    assert_eq!(output.expected_retirement_terminal(&cause), !failure);
    if failure {
        assert!(matches!(output.failure(), Some(OutputError::Identity)));
    }
    assert!(output.terminal.is_some());
    assert_eq!(output.retiring_pull(), Some((h, Moment::from_nanos(100), true)));
    assert!(output.acknowledged.is_none());
    assert_eq!(rig.scope.snapshot().reserved_bytes, SOURCE_BYTES + TRANSFER_BYTES);
    drop(output);
    rig.finish()?;
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn closed_running_worker_cancellation_waits_for_actual_return_and_keeps_original_pull() -> TestResult
{
    worker_boundary(false)
}
#[test]
fn closed_running_worker_keeps_genuine_failure_across_actual_cancellation() -> TestResult {
    worker_boundary(true)
}

#[test]
fn successful_worker_completed_before_close_retains_actual_success_without_ack() -> TestResult {
    let rig = Rig::new()?;
    let mut output = output(&rig)?;
    let h = header(binding()?, 2)?;
    output.prepare_pull(h, Moment::from_nanos(100))?;
    output.admitted_pull(h)?;
    let mut source = Some(output.source.clone());
    output.job = Some(rig.client.submit_blocking_owned_input(
        &rig.scope,
        spec(JobId::new(2)?, Moment::from_nanos(100), TRANSFER_BYTES),
        &mut source,
        |source, context| {
            source.check(&context)?;
            Ok(worker::Parsed { delivery: None, change: None, snapshot: None })
        },
    )?);
    rig.notice(&mut output)?;
    output.close();
    let cause = output.take_applied().err().ok_or("Closed success not retained")?;
    assert!(output.expected_retirement_terminal(&cause));
    assert!(matches!(output.terminal.as_ref().map(|t| &t.outcome), Some(JobOutcome::Success(_))));
    assert!(!output.worker_pending());
    assert!(output.acknowledged.is_none());
    assert_eq!(output.retiring_pull(), Some((h, Moment::from_nanos(100), true)));
    Ok(())
}
