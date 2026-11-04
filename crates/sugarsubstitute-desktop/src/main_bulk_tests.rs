// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise single-use bulk tokens, session fencing, expiry, bounds and ingress reassembly.

use super::*;
use crate::main_panel_ingress::{BINDING_NAME, ClientMessage, Lane, decode_payload, frame};
use std::{cell::Cell, error::Error};
use sugarsubstitute_contracts::{
    canvas_surface::CanvasSurfaceMessage, execution::MemoryRelease, panel::PanelSession,
};

type TestResult = Result<(), Box<dyn Error>>;

const ROOT: &str = "http://127.0.0.1:8188/";
const PAGE: &str = "http://127.0.0.1:8188";
const CONTEXT: i64 = 17;

struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}

/// Deterministic distinct tokens; production draws them from the system generator.
fn transfers(lease: &MemoryLease) -> Result<Transfers, DesktopError> {
    let next = Cell::new(1_u8);
    let transfers = Transfers::new(Endpoint::for_root(ROOT)?, lease.clone(), move || {
        let value = next.get();
        next.set(value.wrapping_add(1));
        Ok([value; 16])
    })?;
    transfers.synchronize(Some(CONTEXT));
    Ok(transfers)
}

fn partition() -> MemoryLease {
    MemoryLease::new(SESSION_BYTES, Box::new(Release))
}

/// Store `body` through the same route, grant and bounded reads as the native handler.
fn upload(transfers: &Transfers, token: Token, body: &[u8], now: Instant) -> Result<(), Refusal> {
    let uri = format!("{}u/{}", transfers.endpoint().prefix(), token.hex());
    let request =
        Request { fetch: true, method: Some("POST"), uri: Some(&uri), origin: Header::Text(PAGE) };
    let Some(Route::Upload(routed)) = transfers.endpoint().route(request) else {
        return Err(Refusal::Closed);
    };
    let slot = transfers.begin_upload(routed).ok_or(Refusal::Closed)?;
    let mut buffer = transfers.upload_buffer();
    let mut offset = 0;
    while buffer.fill(|chunk| {
        let read = chunk.len().min(body.len() - offset).min(256 * 1024);
        chunk[..read].copy_from_slice(&body[offset..offset + read]);
        offset += read;
        Some(read)
    })? {}
    transfers.finish_upload(slot, buffer, now)
}

fn envelope(token: Token, bytes: usize) -> Envelope {
    Envelope { token, bytes }
}

#[test]
fn tokens_round_trip_only_their_canonical_lowercase_spelling() {
    let token = Token::new([0xab; 16]);
    assert_eq!(token.hex(), "ab".repeat(16));
    assert_eq!(Token::parse(&token.hex()), Some(token));
    for invalid in
        ["AB".repeat(16), "ab".repeat(15), format!("{}0", "ab".repeat(16)), "g".repeat(32)]
    {
        assert_eq!(Token::parse(&invalid), None, "{invalid}");
    }
    assert_eq!(format!("{token:?}"), "Token(..)");
}

#[test]
fn envelopes_decode_only_the_exact_private_shape() -> TestResult {
    let token = Token::new([0xc7; 16]);
    let encoded = String::from_utf8(envelope(token, 20_000).encode()?)?;
    assert_eq!(
        encoded,
        format!(
            r#"{{"transport":"sugarsubstitute.bulk","version":1,"token":"{}","bytes":20000}}"#,
            token.hex()
        )
    );
    assert_eq!(Envelope::decode(&encoded), Some(envelope(token, 20_000)));
    let hex = token.hex();
    for invalid in [
        format!(r#"{{"transport":"sugarsubstitute.bulk","version":2,"token":"{hex}","bytes":1}}"#),
        format!(r#"{{"transport":"other","version":1,"token":"{hex}","bytes":1}}"#),
        format!(r#"{{"transport":"sugarsubstitute.bulk","version":1,"token":"{hex}","bytes":0}}"#),
        format!(
            r#"{{"transport":"sugarsubstitute.bulk","version":1,"token":"{hex}","bytes":{}}}"#,
            MAX_MESSAGE_BYTES + 1
        ),
        format!(r#"{{"transport":"sugarsubstitute.bulk","version":1,"token":"{hex}"}}"#),
        format!(
            r#"{{"transport":"sugarsubstitute.bulk","version":1,"token":"{hex}","bytes":1,"x":1}}"#
        ),
        format!(
            r#"{{"transport":"sugarsubstitute.bulk","version":1,"token":"{hex}","token":"{hex}","bytes":1}}"#
        ),
        format!(
            r#"{{"transport":"sugarsubstitute.bulk","version":1,"token":"{}","bytes":1}}"#,
            hex.to_uppercase()
        ),
        r#"{"protocol":"sugarcubes.canvas","version":3,"session":"session-12345678","requestId":1,"type":"context.request"}"#.into(),
    ] {
        assert_eq!(Envelope::decode(&invalid), None, "{invalid}");
    }
    Ok(())
}

#[test]
fn staged_messages_are_single_use_bounded_and_claimed_until_served() -> TestResult {
    let lease = partition();
    let transfers = transfers(&lease)?;
    let now = Instant::now();
    let body = vec![b'x'; 40 * 1024 * 1024];
    let encoded =
        transfers.stage(CONTEXT, body.clone(), now).map_err(|cause| format!("{cause:?}"))?;
    let named = Envelope::decode(std::str::from_utf8(&encoded)?).ok_or("Missing envelope")?;
    assert_eq!(named.bytes, body.len());
    assert!(lease.claim(SESSION_BYTES - body.len() as u64 + 1).is_err());
    let served = transfers.take_download(named.token, now).ok_or("Missing staged message")?;
    assert_eq!(served.bytes(), body.as_slice());
    assert_eq!(served.claimed(), body.len() as u64);
    assert!(transfers.take_download(named.token, now).is_none(), "token replayed");
    drop(served);
    assert!(lease.claim(SESSION_BYTES).is_ok());
    let mut tokens = Vec::new();
    for _ in 0..MAX_DOWNLOADS {
        tokens.push(
            transfers.stage(CONTEXT, vec![b'y'; 8], now).map_err(|cause| format!("{cause:?}"))?,
        );
    }
    assert!(matches!(transfers.stage(CONTEXT, vec![b'y'; 8], now), Err(Refusal::Capacity)));
    assert!(matches!(transfers.stage(CONTEXT, Vec::new(), now), Err(Refusal::Size)));
    assert!(matches!(
        transfers.stage(CONTEXT, vec![b'y'; MAX_MESSAGE_BYTES + 1], now),
        Err(Refusal::Size)
    ));
    assert!(matches!(transfers.stage(CONTEXT + 1, vec![b'y'; 8], now), Err(Refusal::Closed)));
    Ok(())
}

#[test]
fn staged_messages_expire_and_in_flight_bytes_stay_within_the_partition() -> TestResult {
    let lease = partition();
    let transfers = transfers(&lease)?;
    let now = Instant::now();
    let first = transfers
        .stage(CONTEXT, vec![b'a'; MAX_MESSAGE_BYTES], now)
        .map_err(|cause| format!("{cause:?}"))?;
    let first = Envelope::decode(std::str::from_utf8(&first)?).ok_or("Missing envelope")?;
    transfers
        .stage(CONTEXT, vec![b'b'; MAX_MESSAGE_BYTES], now)
        .map_err(|cause| format!("{cause:?}"))?;
    assert!(matches!(transfers.stage(CONTEXT, vec![b'c'; 1], now), Err(Refusal::Memory(_))));
    let later = now + LIFETIME;
    assert!(transfers.take_download(first.token, later).is_none(), "expired token served");
    assert!(lease.claim(SESSION_BYTES).is_ok(), "expired bodies kept their claims");
    Ok(())
}

#[test]
fn a_changed_document_or_closed_window_releases_every_token_and_body() -> TestResult {
    let lease = partition();
    let transfers = transfers(&lease)?;
    let now = Instant::now();
    let staged = transfers.stage(CONTEXT, vec![b'a'; 1024], now).map_err(|c| format!("{c:?}"))?;
    let staged = Envelope::decode(std::str::from_utf8(&staged)?).ok_or("Missing envelope")?;
    let grant = transfers.grant(CONTEXT, Ticket(3))?;
    transfers.synchronize(Some(CONTEXT + 1));
    assert!(lease.claim(SESSION_BYTES).is_ok());
    assert!(transfers.take_download(staged.token, now).is_none());
    assert!(transfers.begin_upload(grant).is_none());
    assert!(transfers.grant(CONTEXT + 1, Ticket(4)).is_err(), "a fenced registry reopened");
    let reopened = self::transfers(&lease)?;
    reopened.grant(CONTEXT, Ticket(5))?;
    reopened.close();
    assert!(reopened.grant(CONTEXT, Ticket(6)).is_err());
    Ok(())
}

#[test]
fn uploads_reassemble_once_through_the_same_lane_framing_and_decode() -> TestResult {
    let lease = partition();
    let transfers = transfers(&lease)?;
    let session = PanelSession::new("session-12345678".into())?;
    let request = format!(
        r#"{{"protocol":"sugarcubes.canvas","version":3,"session":"{}","requestId":1,"type":"surface.prepare","revision":1,"address":{{"sessionId":"canvas:w","viewId":"view:a","windowId":"window:a","attachmentId":"attachment:a","generation":1}}{}}}"#,
        session.as_str(),
        " ".repeat(24 * 1024)
    );
    assert!(request.len() > 16384, "the message must exceed the inline lane");
    let now = Instant::now();
    let token = transfers.grant(CONTEXT, Ticket(42))?;
    upload(&transfers, token, request.as_bytes(), now).map_err(|cause| format!("{cause:?}"))?;
    assert!(lease.claim(SESSION_BYTES).is_err(), "the received body holds no claim");
    let lane = String::from_utf8(envelope(token, request.len()).encode()?)?;
    let event = serde_json::json!({"name": BINDING_NAME, "payload": format!("1:o:42:{lane}"),
        "executionContextId": CONTEXT})
    .to_string();
    let framed = frame(&event, Some(CONTEXT)).ok_or("Envelope exceeded the lane frame")?;
    assert_eq!(framed.lane, Lane::Ordinary);
    let named = Envelope::decode(framed.payload()).ok_or("Missing lane envelope")?;
    let received =
        transfers.take_upload(CONTEXT, framed.ticket, named, now).ok_or("Missing upload")?;
    assert_eq!(received.text(), request);
    let message = decode_payload(received.text(), &session).ok_or("Upload did not decode")?;
    assert!(matches!(message, ClientMessage::Surface(CanvasSurfaceMessage::Request(_))));
    assert_eq!(message.lane(), Lane::Ordinary);
    assert!(transfers.take_upload(CONTEXT, framed.ticket, named, now).is_none(), "replayed");
    drop(received);
    assert!(lease.claim(SESSION_BYTES).is_ok());
    Ok(())
}

#[test]
fn an_uploaded_workflow_sync_decodes_and_keeps_its_body_storage() -> TestResult {
    use sugarsubstitute_contracts::canvas_native::CanvasNativeRequest;
    let lease = partition();
    let transfers = transfers(&lease)?;
    let session = PanelSession::new("session-12345678".into())?;
    let surfaces: Vec<serde_json::Value> = (0..64)
        .map(|index| {
            serde_json::json!({"key": format!("node:{index}:image"), "label": "x".repeat(256),
                "base": {"kind": "blank", "width": 512, "height": 512}, "masks": []})
        })
        .collect();
    let request = serde_json::json!({"protocol": "sugarcubes.canvas", "version": 3,
        "session": session.as_str(), "requestId": 4, "type": "workflow.request",
        "command": {"type": "sync", "workflow": "workflow-1", "surfaces": surfaces}})
    .to_string();
    assert!(request.len() > 16384, "the message must exceed the inline lane");
    let now = Instant::now();
    let token = transfers.grant(CONTEXT, Ticket(8))?;
    upload(&transfers, token, request.as_bytes(), now).map_err(|cause| format!("{cause:?}"))?;
    let named = envelope(token, request.len());
    let received = transfers.take_upload(CONTEXT, Ticket(8), named, now).ok_or("Missing upload")?;
    let message = decode_payload(received.text(), &session).ok_or("Upload did not decode")?;
    assert!(matches!(
        &message,
        ClientMessage::Native(CanvasNativeRequest::Workflow(sync)) if sync.envelope().request_id() == 4
    ));
    assert_eq!(message.lane(), Lane::Ordinary);
    let storage = received.into_claims();
    assert!(!storage.is_empty());
    assert!(lease.claim(SESSION_BYTES).is_err(), "the decoded request released its storage");
    drop(storage);
    assert!(lease.claim(SESSION_BYTES).is_ok());
    Ok(())
}

#[test]
fn upload_tokens_are_single_use_and_bound_to_their_document_ticket_and_size() -> TestResult {
    let lease = partition();
    let transfers = transfers(&lease)?;
    let now = Instant::now();
    let body = vec![b'{'; 20 * 1024 * 1024];
    let token = transfers.grant(CONTEXT, Ticket(7))?;
    upload(&transfers, token, &body, now).map_err(|cause| format!("{cause:?}"))?;
    assert!(transfers.begin_upload(token).is_none(), "upload grant replayed");
    assert!(transfers.take_upload(CONTEXT, Ticket(8), envelope(token, body.len()), now).is_none());
    assert!(
        transfers.take_upload(CONTEXT, Ticket(7), envelope(token, body.len()), now).is_none(),
        "a refused envelope left its token live"
    );
    assert!(lease.claim(SESSION_BYTES).is_ok());

    let token = transfers.grant(CONTEXT, Ticket(9))?;
    upload(&transfers, token, &body, now).map_err(|cause| format!("{cause:?}"))?;
    assert!(
        transfers.take_upload(CONTEXT, Ticket(9), envelope(token, body.len() - 1), now).is_none()
    );

    let token = transfers.grant(CONTEXT, Ticket(10))?;
    assert!(
        transfers.take_upload(CONTEXT, Ticket(10), envelope(token, 1), now).is_none(),
        "an envelope without a body was admitted"
    );
    assert!(transfers.begin_upload(token).is_none(), "an envelope left its grant live");

    let token = transfers.grant(CONTEXT, Ticket(11))?;
    upload(&transfers, token, &body, now).map_err(|cause| format!("{cause:?}"))?;
    let superseding = transfers.grant(CONTEXT, Ticket(12))?;
    assert!(transfers.take_upload(CONTEXT, Ticket(11), envelope(token, body.len()), now).is_none());
    assert!(lease.claim(SESSION_BYTES).is_ok(), "a superseded credit kept its body");

    upload(&transfers, superseding, &body, now).map_err(|cause| format!("{cause:?}"))?;
    let expired = now + LIFETIME;
    let named = envelope(superseding, body.len());
    assert!(transfers.take_upload(CONTEXT, Ticket(12), named, expired).is_none());
    assert!(lease.claim(SESSION_BYTES).is_ok(), "an expired upload kept its claims");
    Ok(())
}

#[test]
fn upload_storage_enforces_the_message_bound_partition_and_utf8_while_reading() -> TestResult {
    let lease = partition();
    let transfers = transfers(&lease)?;
    let now = Instant::now();
    let token = transfers.grant(CONTEXT, Ticket(1))?;
    let oversized = vec![b' '; MAX_MESSAGE_BYTES + 1];
    assert!(matches!(upload(&transfers, token, &oversized, now), Err(Refusal::Size)));
    drop(oversized);
    assert!(lease.claim(SESSION_BYTES).is_ok(), "a refused upload kept its claims");
    let token = transfers.grant(CONTEXT, Ticket(2))?;
    assert!(matches!(upload(&transfers, token, b"{\xff}", now), Err(Refusal::Size)));
    let token = transfers.grant(CONTEXT, Ticket(3))?;
    assert!(matches!(upload(&transfers, token, b"", now), Err(Refusal::Size)));

    let small = MemoryLease::new(UPLOAD_STEP as u64, Box::new(Release));
    let constrained = self::transfers(&small)?;
    let token = constrained.grant(CONTEXT, Ticket(4))?;
    let body = vec![b' '; UPLOAD_STEP + 1];
    assert!(matches!(upload(&constrained, token, &body, now), Err(Refusal::Memory(_))));
    assert!(small.claim(UPLOAD_STEP as u64).is_ok());

    let token = transfers.grant(CONTEXT, Ticket(5))?;
    let slot = transfers.begin_upload(token).ok_or("Missing grant")?;
    let mut buffer = transfers.upload_buffer();
    assert!(matches!(buffer.fill(|_| None), Err(Refusal::Read)));
    assert!(matches!(buffer.fill(|chunk| Some(chunk.len() + 1)), Err(Refusal::Read)));
    drop((slot, buffer));
    assert!(lease.claim(SESSION_BYTES).is_ok());
    Ok(())
}

#[test]
fn generator_failure_refuses_without_issuing_a_token() -> TestResult {
    let lease = partition();
    let transfers = Transfers::new(Endpoint::for_root(ROOT)?, lease, || {
        Err(DesktopError::message("generate_bulk_token", "unavailable"))
    })?;
    transfers.synchronize(Some(CONTEXT));
    assert!(transfers.grant(CONTEXT, Ticket(1)).is_err());
    assert!(matches!(
        transfers.stage(CONTEXT, vec![b'x'; 8], Instant::now()),
        Err(Refusal::Native(_))
    ));
    Ok(())
}
