// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove the loopback server advances on socket events and never on a polling timer.
use super::*;
use crate::runtime_test_support::{Rig, TestResult, spec};
use std::{
    io::{Read, Write},
    net::TcpStream,
    sync::mpsc,
};
use sugarsubstitute_contracts::execution::{ExecutionPort, JobClass, JobOutcome, Moment, OwnedJob};

const REQUEST: &[u8] = b"GET /view HTTP/1.1\r\nHost: fixture\r\n\r\n";
const RESPONSE: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok";
const DEADLINE: Moment = Moment::from_nanos(60_000_000_000);

fn exchange() -> Exchange {
    Exchange {
        path: "/view".into(),
        range: None,
        response: RESPONSE.to_vec(),
        await_client_close: true,
        cancel_client: None,
    }
}

/// Run the scripted server as one async job on the rig's manual clock and return its loopback port.
fn serve(rig: &Rig) -> TestResult<(OwnedJob<(), io::Error>, u16)> {
    let (ports, port) = mpsc::channel();
    let mut bounded = spec(1, JobClass::IoOrdinary, 8 * 1024 * 1024)?;
    bounded.deadline = Some(DEADLINE);
    let job = rig.client.submit_async(&rig.scope, bounded, move |context| async move {
        let server = Server::bind(vec![exchange()], &context)?;
        ports.send(server.port()?).map_err(io::Error::other)?;
        server.run(context).await
    })?;
    Ok((job, port.recv()?))
}

#[test]
fn a_scripted_exchange_completes_while_the_clock_never_advances() -> TestResult {
    let rig = Rig::new()?;
    let (mut job, port) = serve(&rig)?;
    let mut peer = TcpStream::connect(("127.0.0.1", port))?;
    peer.write_all(REQUEST)?;
    let mut response = vec![0; RESPONSE.len()];
    peer.read_exact(&mut response)?;
    assert_eq!(response, RESPONSE);
    drop(peer);
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::Success(())));
    Ok(())
}

#[test]
fn a_request_for_another_target_fails_the_script() -> TestResult {
    let rig = Rig::new()?;
    let (mut job, port) = serve(&rig)?;
    let mut peer = TcpStream::connect(("127.0.0.1", port))?;
    peer.write_all(b"GET /other HTTP/1.1\r\nHost: fixture\r\n\r\n")?;
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::Failure(_)));
    Ok(())
}

#[test]
fn a_silent_peer_ends_only_when_the_job_deadline_is_reached() -> TestResult {
    let rig = Rig::new()?;
    let (mut job, port) = serve(&rig)?;
    let _silent = TcpStream::connect(("127.0.0.1", port))?;
    rig.clock.wait_until_registered(DEADLINE)?;
    rig.clock.advance(59_000_000_000);
    assert!(job.try_take_completion().is_none());
    rig.clock.advance(1_000_000_000);
    let outcome = rig.completion(&mut job)?.outcome;
    assert!(
        matches!(outcome, JobOutcome::TimedOut | JobOutcome::Failure(_)),
        "{:?}",
        outcome.kind()
    );
    Ok(())
}

#[test]
fn cancellation_interrupts_a_server_waiting_for_its_peer() -> TestResult {
    let rig = Rig::new()?;
    let (mut job, port) = serve(&rig)?;
    let _silent = TcpStream::connect(("127.0.0.1", port))?;
    job.cancel();
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::Cancelled));
    Ok(())
}
