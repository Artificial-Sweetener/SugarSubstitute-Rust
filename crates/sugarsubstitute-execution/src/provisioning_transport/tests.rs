// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise original admission, retained delivery and real authenticated loopback teardown.
use super::*;
use crate::runtime_test_support::{Rig, TestResult, spec};
use std::{
    io::{BufRead, BufReader},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use sugarsubstitute_contracts::execution::{
    ExecutionPort, JobClass, JobOutcome, MemoryLease, MemoryRelease, OwnedJob,
};

/// Observe exactly when the original memory ledger is released.
#[derive(Default)]
pub(super) struct Release(Arc<AtomicU64>);
impl MemoryRelease for Release {
    fn release(&self, bytes: u64) {
        self.0.fetch_add(bytes, Ordering::AcqRel);
    }
}

/// Bound both queue storage and allocation growth within the unchanged four MiB grant.
#[test]
fn compiled_protocol_envelope_fits_original_reservation() {
    // One active command may grow from a full allocation to twice that capacity for its newline.
    let commands = (QUEUE_SLOTS + 3) * MAX_FRAME_BYTES;
    // Sixteen retained deliveries, one unpublished frame, and decoder growth overlap.
    let frames = (QUEUE_SLOTS + 3) * MAX_FRAME_BYTES;
    // Authentication constructs two bounded strings and parser scratch, never a JSON value tree.
    let authentication = 8 * MAX_FRAME_BYTES;
    let controls = 256 * 1024
        + 8192
        + (2 * QUEUE_SLOTS + 4) * (size_of::<Frame>() + size_of::<TransportEvent>())
        + size_of::<Transport>()
        + size_of::<TransportJob>();
    assert!(commands + frames + authentication + controls < RESERVATION_BYTES as usize);
}

/// Reject caller allocations that would exceed the transport's retained envelope.
#[test]
fn commands_require_admission_and_reject_excess_capacity() -> TestResult {
    let (transport, _runner) = Transport::pair();
    assert_eq!(
        transport.send("{}".into()).err().map(|error| error.kind()),
        Some(io::ErrorKind::WouldBlock)
    );
    let memory = MemoryLease::new(RESERVATION_BYTES, Box::new(Release::default()));
    transport.budget.activate(&memory)?;
    let mut command = String::with_capacity(MAX_FRAME_BYTES + 1);
    command.push_str("{}");
    assert_eq!(
        transport.send(command).err().map(|error| error.kind()),
        Some(io::ErrorKind::InvalidInput)
    );
    assert_eq!(
        transport.send("{}\n".into()).err().map(|error| error.kind()),
        Some(io::ErrorKind::InvalidInput)
    );
    Ok(())
}

/// A failed original claim cannot activate either command or received-frame ownership.
#[test]
fn insufficient_original_admission_leaves_the_pair_inert() -> TestResult {
    let (transport, runner) = Transport::pair();
    let memory = MemoryLease::new(RESERVATION_BYTES - 1, Box::new(Release::default()));
    assert!(runner.budget.activate(&memory).is_err());
    assert_eq!(
        transport.send("{}".into()).err().map(|cause| cause.kind()),
        Some(io::ErrorKind::WouldBlock)
    );
    assert_eq!(
        runner.budget.permit().err().map(|cause| cause.kind()),
        Some(io::ErrorKind::WouldBlock)
    );
    let claim = memory.claim(RESERVATION_BYTES - 1)?;
    drop(claim);
    Ok(())
}

/// Removing events from a queue cannot mint additional slots while their bytes remain live.
#[test]
fn held_frames_bound_delivery_and_retain_the_original_charge() -> TestResult {
    let release = Release::default();
    let released = release.0.clone();
    let memory = MemoryLease::new(RESERVATION_BYTES, Box::new(release));
    let (transport, runner) = Transport::pair();
    runner.budget.activate(&memory)?;
    let mut frames = Vec::new();
    for _ in 0..QUEUE_SLOTS {
        let frame = Frame { bytes: "作品".into(), _permit: runner.budget.permit()? };
        runner.events.try_send(TransportEvent::Frame(frame)).map_err(|_| "Delivery failed")?;
        let Some(TransportEvent::Frame(frame)) = transport.next_event() else {
            return Err("Expected frame".into());
        };
        frames.push(frame);
    }
    assert_eq!(
        runner.budget.permit().err().map(|error| error.kind()),
        Some(io::ErrorKind::WouldBlock)
    );
    assert!(memory.claim(1).is_err());
    drop(frames.pop());
    let final_frame = Frame { bytes: "retained".into(), _permit: runner.budget.permit()? };
    drop(transport);
    assert_eq!(
        runner.budget.permit().err().map(|error| error.kind()),
        Some(io::ErrorKind::BrokenPipe)
    );
    drop(runner);
    drop(memory);
    drop(frames);
    assert_eq!(released.load(Ordering::Acquire), 0);
    assert_eq!(final_frame.as_str(), "retained");
    drop(final_frame);
    assert_eq!(released.load(Ordering::Acquire), RESERVATION_BYTES);
    Ok(())
}

/// Queue storage remains funded even if its admitted runner has already retired.
#[test]
fn queued_commands_keep_original_credit_until_port_retirement() -> TestResult {
    let release = Release::default();
    let released = release.0.clone();
    let memory = MemoryLease::new(RESERVATION_BYTES, Box::new(release));
    let (transport, runner) = Transport::pair();
    runner.budget.activate(&memory)?;
    transport.send("{}".into())?;
    drop(runner);
    drop(memory);
    assert_eq!(released.load(Ordering::Acquire), 0);
    drop(transport);
    assert_eq!(released.load(Ordering::Acquire), RESERVATION_BYTES);
    Ok(())
}

/// Start the production async job and observe its actual bound listener without polling sleeps.
fn listening(rig: &Rig) -> TestResult<(Transport, OwnedJob<(), io::Error>, TcpStream, String)> {
    let (transport, runner) = Transport::pair();
    let job = rig.client.submit_async(
        &rig.scope,
        spec(1, JobClass::IoOrdinary, RESERVATION_BYTES)?,
        move |context| runner.run(context),
    )?;
    let TransportEvent::Listening { port, token } =
        transport.events.recv_timeout(Duration::from_secs(10))?
    else {
        return Err("Listener did not precede frames".into());
    };
    let peer = TcpStream::connect(("127.0.0.1", port))?;
    peer.set_read_timeout(Some(Duration::from_secs(10)))?;
    peer.set_write_timeout(Some(Duration::from_secs(10)))?;
    Ok((transport, job, peer, token))
}

/// Send actual supported authentication and a burst larger than the consumer-held limit.
fn burst(peer: &mut TcpStream, token: &str) -> io::Result<()> {
    writeln!(peer, "{}", serde_json::json!({"version":1,"event":"authenticate","token":token}))?;
    for index in 0..=QUEUE_SLOTS {
        writeln!(peer, "{}", serde_json::json!({"event":"作品","index":index}))?;
    }
    peer.flush()
}

/// Receive exactly one actual event; the timeout only guards a failed synchronization.
fn receive(transport: &Transport) -> TestResult<Frame> {
    match transport.events.recv_timeout(Duration::from_secs(10))? {
        TransportEvent::Frame(frame) => Ok(frame),
        _ => Err("Unexpected duplicate listener".into()),
    }
}

/// Real socket delivery, Unicode decoding and terminal acknowledgement retain original funding.
#[test]
fn native_frames_survive_successful_job_and_executor_teardown() -> TestResult {
    let rig = Rig::new()?;
    let (transport, mut job, mut peer, token) = listening(&rig)?;
    burst(&mut peer, &token)?;
    transport.send("{\"command\":\"continue\"}".into())?;
    let mut frames = Vec::new();
    for index in 0..QUEUE_SLOTS {
        let frame = receive(&transport)?;
        let value: serde_json::Value = serde_json::from_str(frame.as_str())?;
        assert_eq!(value["event"], "作品");
        assert_eq!(value["index"], index);
        frames.push(frame);
    }
    assert!(transport.next_event().is_none());
    drop(frames.pop());
    frames.push(receive(&transport)?);
    let mut command = String::new();
    BufReader::new(&peer).read_line(&mut command)?;
    assert_eq!(command, "{\"command\":\"continue\"}\n");
    peer.shutdown(Shutdown::Both)?;
    let completion = rig.completion(&mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::Success(())));
    drop(completion);
    drop(job);
    drop(peer);
    drop(transport);
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, RESERVATION_BYTES);
    drop(frames);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

/// Cancellation settles actual I/O while consumer-held bytes remain charged independently.
#[test]
fn native_cancellation_settles_with_every_delivery_slot_held() -> TestResult {
    let rig = Rig::new()?;
    let (transport, mut job, mut peer, token) = listening(&rig)?;
    burst(&mut peer, &token)?;
    let mut frames = Vec::new();
    for _ in 0..QUEUE_SLOTS {
        frames.push(receive(&transport)?);
    }
    job.cancel();
    let completion = rig.completion(&mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::Cancelled));
    drop(completion);
    drop(transport);
    drop(job);
    drop(peer);
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, RESERVATION_BYTES);
    drop(frames);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

/// Closing the host port before authentication retires its real listener/socket job.
#[test]
fn native_host_close_does_not_wait_for_authentication_timeout() -> TestResult {
    let rig = Rig::new()?;
    let (transport, mut job, peer, _token) = listening(&rig)?;
    drop(transport);
    let completion = rig.completion(&mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::Success(())));
    drop(completion);
    drop(job);
    drop(peer);
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

/// A peer without the single-use credential cannot publish a funded application frame.
#[test]
fn native_wrong_credential_is_terminal() -> TestResult {
    let rig = Rig::new()?;
    let (transport, mut job, mut peer, _token) = listening(&rig)?;
    writeln!(peer, "{}", serde_json::json!({"version":1,"event":"authenticate","token":"wrong"}))?;
    let completion = rig.completion(&mut job)?;
    let JobOutcome::Failure(cause) = completion.outcome else {
        return Err("Authentication succeeded".into());
    };
    assert_eq!(cause.kind(), io::ErrorKind::PermissionDenied);
    assert!(transport.next_event().is_none());
    Ok(())
}

/// Rejected bytes and their error alias remain funded after ports, completion and executor retire.
#[test]
fn native_rejected_frame_error_retains_original_credit_and_cause() -> TestResult {
    let rig = Rig::new()?;
    let (transport, mut job, mut peer, token) = listening(&rig)?;
    writeln!(peer, "{}", serde_json::json!({"version":1,"event":"authenticate","token":token}))?;
    peer.write_all(&[0xff; MAX_FRAME_BYTES - 1])?;
    peer.write_all(b"\n")?;
    let completion = rig.completion(&mut job)?;
    let JobOutcome::Failure(cause) = completion.outcome else {
        return Err("Invalid UTF-8 did not fail".into());
    };
    assert_eq!(cause.kind(), io::ErrorKind::Other);
    let alias = cause.into_inner().ok_or("Rejected input lost its cause")?;
    assert!(alias.source().is_some());
    drop(completion.memory);
    drop(job);
    drop(transport);
    drop(peer);
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, RESERVATION_BYTES);
    drop(alias);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
