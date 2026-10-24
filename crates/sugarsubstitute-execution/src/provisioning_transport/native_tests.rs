// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify direct native delivery through the actual authenticated socket and blocking executor.
use super::{tests::*, *};
use crate::runtime_test_support::{Rig, TestResult, spec};
use std::io::{BufRead, BufReader, Write};
use sugarsubstitute_contracts::execution::{ExecutionPort, JobClass, JobContext, JobOutcome};

/// Wait on the production port under the blocking job's actual cancellation and deadline.
fn request(port: &DownloadPort, context: &JobContext) -> io::Result<Frame> {
    loop {
        if let Some(frame) = port.receive(context)? {
            return Ok(frame);
        }
    }
}

/// Native storage exchanges require no UI event polling between authenticated requests and replies.
#[test]
fn native_download_replies_flow_without_ui_polling() -> TestResult {
    let rig = Rig::new()?;
    let (mut transport, mut socket_job, mut peer, token) = listening(&rig)?;
    let port = transport.take_downloads()?;
    let mut service = rig.client.submit_blocking(
        &rig.scope,
        spec(2, JobClass::InfrastructureOrdinary, 4096)?,
        move |context| -> io::Result<_> {
            for serial in 1..=2 {
                let frame = request(&port, &context)?;
                let value: serde_json::Value = serde_json::from_str(frame.as_str())?;
                assert_eq!(value["event"], "downloads");
                assert_eq!(value["serial"], serial);
                port.send(format!("{{\"serial\":{serial}}}"))?;
            }
            Ok(port)
        },
    )?;
    writeln!(peer, "{}", serde_json::json!({"version":1,"event":"authenticate","token":token}))?;
    let mut reader = BufReader::new(peer.try_clone()?);
    for serial in 1..=2 {
        writeln!(
            peer,
            "{}",
            serde_json::json!({"event":"downloads","serial":serial,"path":"作品"})
        )?;
        let mut reply = String::new();
        reader.read_line(&mut reply)?;
        assert_eq!(reply, format!("{{\"serial\":{serial}}}\n"));
    }
    let completion = rig.completion(&mut service)?;
    let JobOutcome::Success(port) = completion.outcome else {
        return Err("Native service did not complete".into());
    };
    assert!(!port.pending());
    assert!(transport.next_event().is_none());
    peer.shutdown(std::net::Shutdown::Both)?;
    assert!(matches!(rig.completion(&mut socket_job)?.outcome, JobOutcome::Success(())));
    Ok(())
}

/// Retaining native deliveries cannot create a second sixteen-slot allowance for UI frames.
#[test]
fn native_and_ui_frames_share_the_original_delivery_limit() -> TestResult {
    let rig = Rig::new()?;
    let (mut transport, mut socket_job, mut peer, token) = listening(&rig)?;
    let port = transport.take_downloads()?;
    let mut service = rig.client.submit_blocking(
        &rig.scope,
        spec(2, JobClass::InfrastructureOrdinary, 4096)?,
        move |context| -> io::Result<_> {
            let mut frames = Vec::with_capacity(QUEUE_SLOTS);
            for _ in 0..QUEUE_SLOTS {
                frames.push(request(&port, &context)?);
            }
            Ok((port, frames))
        },
    )?;
    writeln!(peer, "{}", serde_json::json!({"version":1,"event":"authenticate","token":token}))?;
    for serial in 0..QUEUE_SLOTS {
        writeln!(peer, "{}", serde_json::json!({"event":"downloads","serial":serial}))?;
    }
    writeln!(peer, "{{\"event\":\"progress\"}}")?;
    let completion = rig.completion(&mut service)?;
    let JobOutcome::Success((port, mut frames)) = completion.outcome else {
        return Err("Native deliveries did not arrive".into());
    };
    assert!(transport.next_event().is_none());
    drop(frames.pop());
    assert_eq!(receive(&transport)?.as_str(), "{\"event\":\"progress\"}");
    peer.shutdown(std::net::Shutdown::Both)?;
    assert!(matches!(rig.completion(&mut socket_job)?.outcome, JobOutcome::Success(())));
    drop(transport);
    assert_eq!(rig.scope.snapshot().reserved_bytes, RESERVATION_BYTES + 4096);
    drop(frames);
    drop(port);
    drop(completion.memory);
    drop(service);
    drop(socket_job);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

/// A retained native port grants neither a second receiver nor commands after host retirement.
#[test]
fn native_lane_is_unique_and_cannot_send_after_host_close() -> TestResult {
    let (mut transport, runner) = Transport::pair();
    let port = transport.take_downloads()?;
    assert!(transport.take_downloads().is_err());
    assert_eq!(
        port.send("{}".into()).err().map(|cause| cause.kind()),
        Some(io::ErrorKind::WouldBlock)
    );
    let memory = sugarsubstitute_contracts::execution::MemoryLease::new(
        RESERVATION_BYTES,
        Box::new(Release::default()),
    );
    runner.budget.activate(&memory)?;
    drop(transport);
    assert_eq!(
        port.send("{}".into()).err().map(|cause| cause.kind()),
        Some(io::ErrorKind::BrokenPipe)
    );
    Ok(())
}
