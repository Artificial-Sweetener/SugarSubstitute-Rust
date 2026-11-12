// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Framing and handshake rules, then real loopback sessions against a scripted server.
use super::{frames::Frame, *};
use crate::runtime_test_support::{Rig, TestResult, spec};
use std::{
    io::{Read, Write},
    net::{Ipv4Addr, TcpListener},
};
use sugarsubstitute_contracts::execution::{ExecutionPort, JobClass, JobOutcome, OwnedJob};

fn server(opcode: u8, payload: &[u8]) -> Vec<u8> {
    let mut frame = vec![0x80 | opcode];
    match payload.len() {
        length @ 0..=125 => frame.push(length as u8),
        length @ 126..=0xFFFF => {
            frame.push(126);
            frame.extend_from_slice(&(length as u16).to_be_bytes());
        }
        length => {
            frame.push(127);
            frame.extend_from_slice(&(length as u64).to_be_bytes());
        }
    }
    frame.extend_from_slice(payload);
    frame
}

fn decode(bytes: &[u8], chunk: usize) -> io::Result<Vec<Frame>> {
    let mut decoder = frames::Decoder::default();
    let mut out = Vec::new();
    for piece in bytes.chunks(chunk) {
        decoder.push(piece, &mut out)?;
    }
    Ok(out)
}

#[test]
fn accept_value_matches_the_rfc_example() {
    assert_eq!(
        handshake::expected_accept("dGhlIHNhbXBsZSBub25jZQ=="),
        "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
    );
    assert_eq!(handshake::key(b"the sample nonce"), "dGhlIHNhbXBsZSBub25jZQ==");
}

#[test]
fn upgrade_requires_101_the_upgrade_headers_and_the_matching_accept() {
    let key = "dGhlIHNhbXBsZSBub25jZQ==";
    let good = "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: keep-alive, Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n";
    assert!(handshake::verify(good.as_bytes(), key).is_ok());
    for bad in [
        good.replace("101", "200"),
        good.replace("Upgrade: websocket\r\n", ""),
        good.replace("s3pPLMBiTxaQ9kYGzzhZRbK+xOo=", "wrong"),
    ] {
        assert_eq!(
            handshake::verify(bad.as_bytes(), key).map_err(|cause| cause.kind()),
            Err(io::ErrorKind::ConnectionRefused)
        );
    }
}

#[test]
fn fragmented_text_reassembles_across_any_read_boundary() -> io::Result<()> {
    let mut bytes = vec![0x01, 3];
    bytes.extend_from_slice(&"ステ".as_bytes()[..3]);
    bytes.push(0x80);
    bytes.push(3);
    bytes.extend_from_slice(&"ステ".as_bytes()[3..]);
    for chunk in [1, 2, 7, 64] {
        assert_eq!(decode(&bytes, chunk)?, vec![Frame::Text("ステ".into())]);
    }
    Ok(())
}

#[test]
fn binary_and_oversized_text_are_discarded_with_their_length() -> io::Result<()> {
    let mut bytes = server(0x2, &[7; 300]);
    bytes.extend(server(0x1, &vec![b'a'; MAX_MESSAGE_BYTES + 1]));
    bytes.extend(server(0x1, b"kept"));
    assert_eq!(
        decode(&bytes, 4096)?,
        vec![
            Frame::Discarded { bytes: 300, binary: true },
            Frame::Discarded { bytes: MAX_MESSAGE_BYTES as u64 + 1, binary: false },
            Frame::Text("kept".into()),
        ]
    );
    Ok(())
}

#[test]
fn control_frames_interleave_and_protocol_violations_reject() -> io::Result<()> {
    let mut bytes = vec![0x01, 2, b'h', b'i'];
    bytes.extend(server(0x9, b"p"));
    bytes.extend([0x80, 1, b'!']);
    bytes.extend(server(0x8, &[0x03, 0xE9, b'o', b'k']));
    assert_eq!(
        decode(&bytes, 3)?,
        vec![Frame::Ping(b"p".to_vec()), Frame::Text("hi!".into()), Frame::Close(Some(1001))]
    );
    for bad in [
        vec![0x81, 0x81, 0, 0, 0, 0, b'x'],
        vec![0xC1, 0],
        vec![0x80, 0],
        vec![0x09, 0],
        vec![0x83, 0],
        vec![0x81, 1, 0xFF],
    ] {
        assert_eq!(decode(&bad, 8).map_err(|cause| cause.kind()), Err(io::ErrorKind::InvalidData));
    }
    Ok(())
}

#[test]
fn client_frames_are_masked_and_targets_are_loopback_origin_forms() -> io::Result<()> {
    let frame = frames::masked(0xA, b"abc", [1, 2, 3, 4]);
    assert_eq!(frame, vec![0x8A, 0x83, 1, 2, 3, 4, b'a' ^ 1, b'b' ^ 2, b'c' ^ 3]);
    let loopback = SocketAddr::from((Ipv4Addr::LOCALHOST, 8188));
    Target::new(loopback, "/ws?clientId=abc")?;
    for (address, path) in [
        (SocketAddr::from((Ipv4Addr::new(192, 168, 1, 2), 8188)), "/ws"),
        (SocketAddr::from((Ipv4Addr::LOCALHOST, 0)), "/ws"),
        (loopback, "ws"),
        (loopback, "//evil/ws"),
        (loopback, "/ws path"),
        (loopback, "/ws\r\nX: y"),
    ] {
        assert!(Target::new(address, path).is_err());
    }
    Ok(())
}

/// A scripted server: accept, answer the upgrade, then run `script` on the connection.
struct Scripted {
    listener: TcpListener,
}
impl Scripted {
    fn new() -> io::Result<Self> {
        Ok(Self { listener: TcpListener::bind((Ipv4Addr::LOCALHOST, 0))? })
    }
    fn target(&self) -> io::Result<Target> {
        Target::new(self.listener.local_addr()?, "/ws?clientId=test")
    }
    fn accept(&self, accept_override: Option<&str>) -> TestResult<std::net::TcpStream> {
        let (mut peer, _) = self.listener.accept()?;
        let mut head = Vec::new();
        let mut byte = [0];
        while handshake::head_length(&head).is_none() {
            peer.read_exact(&mut byte)?;
            head.push(byte[0]);
        }
        let head = String::from_utf8(head)?;
        assert!(head.starts_with("GET /ws?clientId=test HTTP/1.1\r\n"));
        let key = head
            .lines()
            .find_map(|line| line.strip_prefix("Sec-WebSocket-Key: "))
            .ok_or("key absent")?;
        let accept = accept_override.map_or_else(|| handshake::expected_accept(key), str::to_owned);
        write!(
            peer,
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
        )?;
        Ok(peer)
    }
}

/// Read one masked client frame and return its opcode and unmasked payload.
fn client_frame(peer: &mut std::net::TcpStream) -> TestResult<(u8, Vec<u8>)> {
    let mut head = [0; 2];
    peer.read_exact(&mut head)?;
    assert_eq!(head[1] & 0x80, 0x80, "client frames are masked");
    let length = usize::from(head[1] & 0x7F);
    let mut mask = [0; 4];
    peer.read_exact(&mut mask)?;
    let mut payload = vec![0; length];
    peer.read_exact(&mut payload)?;
    payload.iter_mut().enumerate().for_each(|(index, byte)| *byte ^= mask[index % 4]);
    Ok((head[0] & 0x0F, payload))
}

type Watch = OwnedJob<(Ended<String>, Vec<String>), io::Error>;

fn submit(rig: &Rig, id: u64, target: Target) -> TestResult<Watch> {
    Ok(rig.client.submit_async(
        &rig.scope,
        spec(id, JobClass::IoOrdinary, RESERVATION_BYTES)?,
        move |context| async move {
            let until =
                context.now().checked_add(Duration::from_secs(3600)).map_err(io::Error::other)?;
            let mut seen = Vec::new();
            let ended = watch(&target, &context, until, |incoming| match incoming {
                Incoming::Text(text) if text == "done" => Some(text.to_owned()),
                other => {
                    seen.push(format!("{other:?}"));
                    None
                }
            })
            .await?;
            Ok((ended, seen))
        },
    )?)
}

#[test]
fn session_answers_pings_reports_discards_and_returns_the_observed_value() -> TestResult {
    let rig = Rig::new()?;
    let server_side = Scripted::new()?;
    let mut job = submit(&rig, 1, server_side.target()?)?;
    let mut peer = server_side.accept(None)?;
    peer.write_all(&server(0x9, b"beat"))?;
    assert_eq!(client_frame(&mut peer)?, (0xA, b"beat".to_vec()));
    peer.write_all(&server(0x2, &[0; 10]))?;
    peer.write_all(&server(0x1, b"waiting"))?;
    peer.write_all(&server(0x1, b"done"))?;
    let completion = rig.completion(&mut job)?;
    let JobOutcome::Success((ended, seen)) = completion.outcome else {
        return Err("watch did not succeed".into());
    };
    assert_eq!(ended, Ended::Observed("done".into()));
    assert_eq!(
        seen,
        vec!["Discarded { bytes: 10, binary: true }".to_owned(), "Text(\"waiting\")".to_owned(),]
    );
    Ok(())
}

#[test]
fn server_close_is_echoed_and_a_wrong_accept_refuses_the_upgrade() -> TestResult {
    let rig = Rig::new()?;
    let server_side = Scripted::new()?;
    let mut job = submit(&rig, 2, server_side.target()?)?;
    let mut peer = server_side.accept(None)?;
    peer.write_all(&server(0x8, &1001_u16.to_be_bytes()))?;
    assert_eq!(client_frame(&mut peer)?, (0x8, 1001_u16.to_be_bytes().to_vec()));
    let completion = rig.completion(&mut job)?;
    assert!(matches!(
        completion.outcome,
        JobOutcome::Success((Ended::ServerClosed(Some(1001)), _))
    ));

    let mut job = submit(&rig, 3, server_side.target()?)?;
    let _peer = server_side.accept(Some("bm90IHRoZSBhY2NlcHQ="))?;
    let completion = rig.completion(&mut job)?;
    let JobOutcome::Failure(cause) = completion.outcome else {
        return Err("a wrong accept value was not refused".into());
    };
    assert_eq!(cause.kind(), io::ErrorKind::ConnectionRefused);
    Ok(())
}

#[test]
fn a_lost_peer_fails_the_watch_and_cancellation_releases_the_reservation() -> TestResult {
    let rig = Rig::new()?;
    let server_side = Scripted::new()?;
    let mut job = submit(&rig, 4, server_side.target()?)?;
    drop(server_side.accept(None)?);
    let completion = rig.completion(&mut job)?;
    let JobOutcome::Failure(cause) = &completion.outcome else {
        return Err("a vanished server was not a failure".into());
    };
    assert_eq!(cause.kind(), io::ErrorKind::UnexpectedEof);
    drop(completion);

    let mut job = submit(&rig, 5, server_side.target()?)?;
    let _peer = server_side.accept(None)?;
    rig.scope.cancel();
    let completion = rig.completion(&mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::Cancelled));
    drop(completion);
    drop(job);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
