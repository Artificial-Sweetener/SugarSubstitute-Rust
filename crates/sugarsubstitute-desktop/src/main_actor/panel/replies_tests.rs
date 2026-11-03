// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise response ordering, backpressure and lifetime without a native browser.

use super::*;
use crate::{
    CanvasNativeReply, MainIdentity, SurfaceReply,
    main_events::{Command, Events, Kind},
};
use serde_json::{Value, json};
use std::error::Error;
use sugarsubstitute_contracts::{
    canvas_native::{CanvasContextRequest, CanvasNativeCapabilities},
    canvas_surface::{CanvasSurfaceMessage, CanvasSurfaceReplyStatus},
    execution::{Generation, MemoryLease, MemoryRelease},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    panel::PanelSession,
    startup::LaunchId,
};

struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}

const PANEL_BACKLOG: usize = 4;

fn identity() -> Result<MainIdentity, Box<dyn Error>> {
    Ok(MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([7; 32]),
                epoch: CoordinatorEpoch::new([8; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        },
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    })
}

fn context(
    identity: MainIdentity,
    id: u64,
    lease: &MemoryLease,
) -> Result<NativeDelivery, Box<dyn Error>> {
    let session = PanelSession::for_main(identity)?;
    let request = CanvasContextRequest::decode(&serde_json::to_vec(&json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":session.as_str(),
        "requestId":id, "type":"context.request"
    }))?)?;
    let capabilities = CanvasNativeCapabilities { display_output: true, ..Default::default() };
    Ok(NativeDelivery::Canvas(CanvasNativeReply::new(
        identity,
        request.reply(capabilities, None)?,
        lease.claim(CanvasNativeReply::STORAGE_BYTES)?,
    )?))
}

fn surface(
    identity: MainIdentity,
    id: u64,
    lease: &MemoryLease,
) -> Result<NativeDelivery, Box<dyn Error>> {
    let session = PanelSession::for_main(identity)?;
    let message = CanvasSurfaceMessage::decode(&serde_json::to_vec(&json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":session.as_str(),
        "requestId":id, "type":"surface.prepare", "revision":id,
        "address":{"sessionId":"canvas:workflow", "viewId":"view:one",
            "windowId":"window:one", "attachmentId":"attachment:one", "generation":1}
    }))?)?;
    let CanvasSurfaceMessage::Request(request) = message else {
        return Err("Expected a surface request".into());
    };
    Ok(NativeDelivery::Surface(SurfaceReply::new(
        identity,
        &request,
        CanvasSurfaceReplyStatus::Ready,
        "",
        lease.claim(SurfaceReply::STORAGE_BYTES)?,
    )?))
}

fn record_panel(events: &Events, session: &PanelSession, request: u64) {
    let payload = serde_json::json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":session.as_str(),
        "requestId":request, "type":"cancel"
    })
    .to_string();
    let event = serde_json::json!({
        "name":crate::main_panel_ingress::BINDING_NAME,
        "payload":payload,
        "executionContextId":17
    })
    .to_string();
    events.record(Kind::PanelBinding, |target| {
        target.push_str(&event);
        Ok(())
    });
}

#[test]
fn replenished_panel_evidence_cannot_starve_real_reply_custody() -> Result<(), Box<dyn Error>> {
    let identity = identity()?;
    let session = PanelSession::for_main(identity)?;
    let lease = MemoryLease::new(4 * 1024 * 1024, Box::new(Release));
    let events = Events::new(&lease, || {})?;
    for request in 1..=PANEL_BACKLOG as u64 {
        record_panel(&events, &session, request);
    }
    let mut replies = Replies::new();
    for request in 1..=CAPACITY as u64 {
        replies.push(surface(identity, request, &lease)?)?;
    }

    let mut consumed = 0_u64;
    let mut sent = Vec::new();
    let mut first_reply_turn = None;
    for turn in 1..=64 {
        if replies.reply_due()
            && replies.progress(true, |reply| {
                assert_eq!(reply.identity(), identity);
                let message: Value = serde_json::from_slice(
                    reply
                        .payload()
                        .ok_or_else(|| DesktopError::message("test_reply", "missing payload"))?,
                )
                .map_err(|cause| DesktopError::new("test_reply", cause))?;
                sent.push(message["requestId"].as_u64().ok_or_else(|| {
                    DesktopError::message("test_reply", "request identity is missing")
                })?);
                Ok(())
            })?
        {
            first_reply_turn.get_or_insert(turn);
            continue;
        }
        assert!(events.consume(|observation| {
            assert_eq!(observation.kind, Kind::PanelBinding);
            Ok(())
        })?);
        consumed += 1;
        replies.evidence_serviced();
        record_panel(&events, &session, 4 + consumed);
    }

    assert_eq!(first_reply_turn, Some(2));
    assert_eq!(sent, (1..=CAPACITY as u64).collect::<Vec<_>>());
    assert_eq!(consumed, 64 - CAPACITY as u64);
    assert_eq!(events.remaining(), PANEL_BACKLOG);
    assert!(replies.is_empty());
    Ok(())
}

#[test]
fn busy_reply_turn_services_real_completion_evidence_before_retry() -> Result<(), Box<dyn Error>> {
    let identity = identity()?;
    let lease = MemoryLease::new(2 * 1024 * 1024, Box::new(Release));
    let events = Events::new(&lease, || {})?;
    let mut replies = Replies::new();
    replies.push(context(identity, 91, &lease)?)?;
    replies.evidence_serviced();
    events.record_command(Command::PanelDelivery(17), |payload| {
        payload.push_str(r#"{"result":{"type":"undefined"}}"#);
        Ok(())
    });

    let mut sent = Vec::new();
    assert!(replies.reply_due());
    assert!(!replies.progress(false, |_| {
        sent.push(0);
        Ok(())
    })?);
    assert!(events.consume(|observation| {
        assert_eq!(observation.kind, Kind::Command);
        assert_eq!(observation.command, Some(Command::PanelDelivery(17)));
        Ok(())
    })?);
    replies.evidence_serviced();
    assert!(replies.progress(true, |reply| {
        assert_eq!(reply.identity(), identity);
        let message: Value = serde_json::from_slice(
            reply
                .payload()
                .ok_or_else(|| DesktopError::message("test_reply", "missing payload"))?,
        )
        .map_err(|cause| DesktopError::new("test_reply", cause))?;
        sent.push(
            message["requestId"].as_u64().ok_or_else(|| {
                DesktopError::message("test_reply", "request identity is missing")
            })?,
        );
        Ok(())
    })?);
    assert_eq!(sent, [91]);
    assert!(replies.is_empty());
    Ok(())
}

#[test]
fn busy_delivery_retains_original_claims_and_sends_each_reply_in_order()
-> Result<(), Box<dyn Error>> {
    let identity = identity()?;
    let lease = MemoryLease::new(2 * SurfaceReply::STORAGE_BYTES, Box::new(Release));
    let mut queue = Replies::new();
    queue.push(context(identity, 1, &lease)?)?;
    queue.push(surface(identity, 2, &lease)?)?;
    let mut sent = Vec::new();
    assert!(!queue.progress(false, |_| {
        sent.push(0);
        Ok(())
    })?);
    assert!(sent.is_empty());
    assert!(lease.claim(1).is_err());
    for expected in [1, 2] {
        assert!(queue.progress(true, |reply| {
            assert_eq!(reply.identity(), identity);
            let message: Value = serde_json::from_slice(
                reply
                    .payload()
                    .ok_or_else(|| DesktopError::message("test_reply", "missing payload"))?,
            )
            .map_err(|cause| DesktopError::new("test_reply", cause))?;
            assert_eq!(message["requestId"], expected);
            sent.push(expected);
            Ok(())
        })?);
    }
    assert_eq!(sent, [1, 2]);
    assert!(queue.is_empty());
    assert!(!queue.progress(true, |_| Ok(()))?);
    assert!(lease.claim(2 * SurfaceReply::STORAGE_BYTES).is_ok());
    Ok(())
}

#[test]
fn failed_submission_transfers_original_custody_to_its_native_owner() -> Result<(), Box<dyn Error>>
{
    let identity = identity()?;
    let lease = MemoryLease::new(SurfaceReply::STORAGE_BYTES, Box::new(Release));
    let mut queue = Replies::new();
    queue.push(surface(identity, 3, &lease)?)?;
    let mut native = None;
    assert!(
        queue
            .progress(true, |reply| {
                native = Some(reply);
                Err(DesktopError::message("test", "submission failed"))
            })
            .is_err()
    );
    assert!(queue.is_empty());
    assert!(lease.claim(1).is_err());
    queue.clear();
    assert!(queue.is_empty());
    assert!(lease.claim(1).is_err());
    drop(native);
    assert!(lease.claim(SurfaceReply::STORAGE_BYTES).is_ok());
    Ok(())
}

#[test]
fn overload_is_explicit_and_close_releases_every_retained_reply() -> Result<(), Box<dyn Error>> {
    let identity = identity()?;
    let lease =
        MemoryLease::new((CAPACITY as u64 + 1) * SurfaceReply::STORAGE_BYTES, Box::new(Release));
    let mut queue = Replies::new();
    for id in 1..=CAPACITY as u64 {
        queue.push(context(identity, id, &lease)?)?;
    }
    assert!(queue.push(context(identity, CAPACITY as u64 + 1, &lease)?).is_err());
    assert!(!queue.is_empty());
    queue.clear();
    assert!(queue.is_empty());
    assert!(lease.claim((CAPACITY as u64 + 1) * SurfaceReply::STORAGE_BYTES).is_ok());
    Ok(())
}

#[test]
fn canvas_notifications_coalesce_by_kind_and_retire_with_their_navigation()
-> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::canvas_native::{CanvasRevision, CanvasSourceChange};
    let identity = identity()?;
    let session = PanelSession::for_main(identity)?;
    let lease =
        MemoryLease::new(4 * crate::CanvasNativeNotification::STORAGE_BYTES, Box::new(Release));
    let notification = |revision: &str| -> Result<NativeDelivery, Box<dyn Error>> {
        let change =
            CanvasSourceChange { document: None, revision: CanvasRevision::new(revision)? };
        Ok(NativeDelivery::CanvasNotification(crate::CanvasNativeNotification::new(
            identity,
            change.encode(&session)?,
            lease.claim(crate::CanvasNativeNotification::STORAGE_BYTES)?,
        )?))
    };
    let mut queue = Replies::new();
    queue.push(notification("1")?)?;
    queue.push(context(identity, 5, &lease)?)?;
    queue.push(notification("2")?)?;
    // The superseded notification released its claim; the reply keeps its ordinary credit.
    assert!(lease.claim(crate::CanvasNativeNotification::STORAGE_BYTES).is_ok());
    let mut sent = Vec::new();
    while queue.progress(true, |delivery| {
        let message: Value = serde_json::from_slice(
            delivery
                .payload()
                .ok_or_else(|| DesktopError::message("test_reply", "missing payload"))?,
        )
        .map_err(|cause| DesktopError::new("test_reply", cause))?;
        sent.push((message["type"].clone(), delivery.lane().is_some()));
        Ok(())
    })? {}
    assert_eq!(sent, [(json!("source.changed"), false), (json!("context.reply"), true)]);
    queue.push(notification("3")?)?;
    let foreign = MainIdentity { navigation: Generation::new(2)?, ..identity };
    assert!(queue.retire_presentations(Some(foreign))?);
    assert!(queue.is_empty());
    assert!(lease.claim(lease.bytes()).is_ok());
    Ok(())
}

#[path = "replies_presentation_tests.rs"]
mod presentation;
