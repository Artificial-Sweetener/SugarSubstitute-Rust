// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The coordinator's update status and the child's restart-to-apply intent on the wire.
use super::*;
use std::num::NonZeroU64;

#[test]
fn update_status_and_restart_intent_round_trip_with_their_identity_rules() -> TestResult {
    let revision = NonZeroU64::new(5).ok_or("zero revision")?;
    let status = br#"{"activity":null}"#;
    let source =
        encoded(WirePayload::UpdateStatus { revision, workflow: WorkflowQueue::Busy, status })?;
    let decoded = FrameCodec::decode(&source)?;
    assert!(decoded.header.request.is_none());
    let DecodedPayload::UpdateStatus { revision: read, workflow, status: bytes } = decoded.payload
    else {
        return Err("update status changed kind".into());
    };
    assert_eq!((read, workflow, bytes), (revision, WorkflowQueue::Busy, status.as_slice()));
    for queue in [WorkflowQueue::Idle, WorkflowQueue::Unobserved] {
        let source = encoded(WirePayload::UpdateStatus { revision, workflow: queue, status })?;
        let decoded = FrameCodec::decode(&source)?;
        assert!(
            matches!(decoded.payload, DecodedPayload::UpdateStatus { workflow, .. } if workflow == queue)
        );
    }

    let source = encoded(WirePayload::RestartToApply)?;
    let decoded = FrameCodec::decode(&source)?;
    assert!(decoded.header.request.is_some());
    assert!(matches!(decoded.payload, DecodedPayload::RestartToApply));

    let mut with_request = header(22)?;
    with_request.request = Some(RequestId::new(13)?);
    let message = WireMessage {
        header: with_request,
        payload: WirePayload::UpdateStatus { revision, workflow: WorkflowQueue::Idle, status },
    };
    assert!(matches!(FrameCodec::encode(&message, &mut [0; 256]), Err(ProtocolError::Identity)));
    Ok(())
}

#[test]
fn malformed_update_status_frames_reject() -> TestResult {
    let revision = NonZeroU64::new(1).ok_or("zero revision")?;
    let source = encoded(WirePayload::UpdateStatus {
        revision,
        workflow: WorkflowQueue::Idle,
        status: b"{}",
    })?;
    let payload = HEADER_BYTES;
    let mut zero_revision = source.clone();
    zero_revision[payload..payload + 8].copy_from_slice(&0u64.to_le_bytes());
    assert!(FrameCodec::decode(&zero_revision).is_err());
    let mut unknown_activity = source.clone();
    unknown_activity[payload + 8] = 4;
    assert!(matches!(FrameCodec::decode(&unknown_activity), Err(ProtocolError::UnsupportedTag)));
    let mut overlong = source.clone();
    overlong[payload + 9..payload + 13]
        .copy_from_slice(&(MAX_UPDATE_STATUS_BYTES as u32 + 1).to_le_bytes());
    assert!(matches!(FrameCodec::decode(&overlong), Err(ProtocolError::Length)));
    let mut trailing = encoded(WirePayload::RestartToApply)?;
    trailing.push(0);
    set_length(&mut trailing);
    assert!(matches!(FrameCodec::decode(&trailing), Err(ProtocolError::TrailingBytes)));
    let oversized = vec![b' '; MAX_UPDATE_STATUS_BYTES + 1];
    let message = WireMessage {
        header: header(22)?,
        payload: WirePayload::UpdateStatus {
            revision,
            workflow: WorkflowQueue::Idle,
            status: &oversized,
        },
    };
    assert!(matches!(
        FrameCodec::encode(&message, &mut vec![0; MAX_FRAME_BYTES]),
        Err(ProtocolError::Length)
    ));
    Ok(())
}

#[test]
fn a_sugarcubes_page_failure_round_trips_as_a_child_request_and_unknown_causes_reject() -> TestResult
{
    for failure in [PageFailure::Unavailable, PageFailure::SetupFailed] {
        let source = encoded(WirePayload::PageFailed(failure))?;
        let decoded = FrameCodec::decode(&source)?;
        assert!(decoded.header.request.is_some(), "a page failure is a child request");
        assert!(matches!(decoded.payload, DecodedPayload::PageFailed(read) if read == failure));
    }
    let mut unknown = encoded(WirePayload::PageFailed(PageFailure::Unavailable))?;
    unknown[HEADER_BYTES] = 3;
    assert!(matches!(FrameCodec::decode(&unknown), Err(ProtocolError::UnsupportedTag)));
    let mut trailing = encoded(WirePayload::PageFailed(PageFailure::SetupFailed))?;
    trailing.push(0);
    set_length(&mut trailing);
    assert!(matches!(FrameCodec::decode(&trailing), Err(ProtocolError::TrailingBytes)));
    let mut without_request = header(24)?;
    without_request.request = None;
    let message = WireMessage {
        header: without_request,
        payload: WirePayload::PageFailed(PageFailure::Unavailable),
    };
    assert!(matches!(FrameCodec::encode(&message, &mut [0; 256]), Err(ProtocolError::Identity)));
    Ok(())
}
