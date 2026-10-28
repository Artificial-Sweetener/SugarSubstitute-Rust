// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove panel command coalescing and original admitted-memory custody.

use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

struct Release(Arc<AtomicU64>);
impl MemoryRelease for Release {
    fn release(&self, bytes: u64) {
        self.0.fetch_add(bytes, Ordering::SeqCst);
    }
}

fn identity() -> Result<MainIdentity> {
    Ok(MainIdentity {
        run: super::super::test_support::run()?,
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    })
}

fn surface_request(session: &PanelSession, request_id: u64) -> Result<CanvasSurfaceMessage> {
    let payload = serde_json::to_vec(&serde_json::json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":session.as_str(),
        "requestId":request_id, "type":"surface.prepare", "revision":3,
        "address": {"sessionId":"canvas:workflow", "viewId":"view:one",
            "windowId":"window:one", "attachmentId":"attachment:one", "generation":1}
    }))?;
    Ok(CanvasSurfaceMessage::decode(&payload)?)
}

fn context_request(session: &PanelSession) -> Result<CanvasContextRequest> {
    Ok(CanvasContextRequest::decode(&serde_json::to_vec(&serde_json::json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":session.as_str(),
        "requestId":9, "type":"context.request"
    }))?)?)
}

#[test]
fn surface_negotiation_reports_unavailable_and_retains_the_ingress_claim() -> Result<()> {
    let released = Arc::new(AtomicU64::new(0));
    let reserved = OWNER_BYTES + SurfaceReply::STORAGE_BYTES;
    let lease = MemoryLease::new(reserved, Box::new(Release(released.clone())));
    let identity = identity()?;
    let session = PanelSession::for_main(identity)?;
    let mut panel = PanelOwner::new(identity, lease.clone(), Moment::from_nanos(0))?
        .ok_or("Panel owner was not admitted")?;
    let claim = lease.claim(SurfaceReply::STORAGE_BYTES)?;
    panel.receive_surface(identity, surface_request(&session, 7)?, &lease, claim)?;
    let Some(DesktopStartupCommand::SendSurface(reply)) = panel.take_canvas_reply() else {
        return Err("Surface reply missing".into());
    };
    let payload: serde_json::Value = serde_json::from_slice(reply.payload())?;
    assert_eq!(payload["status"], "unavailable");
    assert_eq!(payload["requestId"], 7);
    assert_eq!(payload["revision"], 3);
    assert_eq!(payload["address"]["viewId"], "view:one");
    drop(panel);
    drop(lease);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    drop(reply);
    assert_eq!(released.load(Ordering::SeqCst), reserved);
    Ok(())
}

#[test]
fn surface_negotiation_rejects_foreign_navigation_and_memory_custody() -> Result<()> {
    let reserved = OWNER_BYTES + SurfaceReply::STORAGE_BYTES;
    let lease = MemoryLease::new(reserved, Box::new(Release(Arc::default())));
    let other = MemoryLease::new(reserved, Box::new(Release(Arc::default())));
    let identity = identity()?;
    let session = PanelSession::for_main(identity)?;
    let mut panel = PanelOwner::new(identity, lease.clone(), Moment::from_nanos(0))?
        .ok_or("Panel owner was not admitted")?;
    let foreign = MainIdentity { navigation: Generation::new(2)?, ..identity };
    assert!(
        panel
            .receive_surface(
                foreign,
                surface_request(&session, 1)?,
                &lease,
                lease.claim(SurfaceReply::STORAGE_BYTES)?,
            )
            .is_err()
    );
    assert!(
        panel
            .receive_surface(
                identity,
                surface_request(&session, 2)?,
                &other,
                other.claim(SurfaceReply::STORAGE_BYTES)?,
            )
            .is_err()
    );
    let foreign_session = PanelSession::for_main(foreign)?;
    assert!(
        panel
            .receive_surface(
                identity,
                surface_request(&foreign_session, 3)?,
                &lease,
                lease.claim(SurfaceReply::STORAGE_BYTES)?,
            )
            .is_err()
    );
    assert!(panel.take_canvas_reply().is_none());
    Ok(())
}

#[test]
fn canvas_context_reports_only_the_currently_implemented_display_capabilities() -> Result<()> {
    let released = Arc::new(AtomicU64::new(0));
    let reserved = OWNER_BYTES + CanvasContextReply::STORAGE_BYTES;
    let lease = MemoryLease::new(reserved, Box::new(Release(released.clone())));
    let identity = identity()?;
    let session = PanelSession::for_main(identity)?;
    let mut panel = PanelOwner::new(identity, lease.clone(), Moment::from_nanos(0))?
        .ok_or("Panel owner was not admitted")?;
    let request = context_request(&session)?;
    panel.receive_context(
        identity,
        request,
        &lease,
        lease.claim(CanvasContextReply::STORAGE_BYTES)?,
    )?;
    let Some(DesktopStartupCommand::SendCanvasContext(reply)) = panel.take_canvas_reply() else {
        return Err("Canvas context reply missing".into());
    };
    let value: serde_json::Value = serde_json::from_slice(reply.payload())?;
    assert_eq!(value["requestId"], 9);
    assert_eq!(value["session"], session.as_str());
    assert_eq!(value["context"]["canvas"]["capabilities"]["displayOutput"], false);
    assert_eq!(value["context"]["canvas"]["capabilities"]["insertImageLayer"], false);
    assert_eq!(value["context"]["document"], serde_json::Value::Null);
    drop(panel);
    drop(lease);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    drop(reply);
    assert_eq!(released.load(Ordering::SeqCst), reserved);
    Ok(())
}

#[test]
fn canvas_context_rejects_foreign_navigation_and_memory_custody() -> Result<()> {
    let reserved = OWNER_BYTES + CanvasContextReply::STORAGE_BYTES;
    let lease = MemoryLease::new(reserved, Box::new(Release(Arc::default())));
    let other = MemoryLease::new(reserved, Box::new(Release(Arc::default())));
    let identity = identity()?;
    let session = PanelSession::for_main(identity)?;
    let mut panel = PanelOwner::new(identity, lease.clone(), Moment::from_nanos(0))?
        .ok_or("Panel owner was not admitted")?;
    let foreign = MainIdentity { navigation: Generation::new(2)?, ..identity };
    assert!(
        panel
            .receive_context(
                foreign,
                context_request(&session)?,
                &lease,
                lease.claim(CanvasContextReply::STORAGE_BYTES)?,
            )
            .is_err()
    );
    assert!(
        panel
            .receive_context(
                identity,
                context_request(&session)?,
                &other,
                other.claim(CanvasContextReply::STORAGE_BYTES)?,
            )
            .is_err()
    );
    assert!(
        panel
            .receive_context(
                identity,
                context_request(&PanelSession::for_main(foreign)?)?,
                &lease,
                lease.claim(CanvasContextReply::STORAGE_BYTES)?,
            )
            .is_err()
    );
    assert!(panel.take_canvas_reply().is_none());
    Ok(())
}

#[test]
fn current_panel_coalesces_pending_messages_and_keeps_memory_until_the_command_retires()
-> Result<()> {
    let released = Arc::new(AtomicU64::new(0));
    let reserved = OWNER_BYTES + 2 * PanelCommand::STORAGE_BYTES;
    let lease = MemoryLease::new(reserved, Box::new(Release(released.clone())));
    let identity = identity()?;
    let session = PanelSession::for_main(identity)?;
    let mut panel = PanelOwner::new(identity, lease.clone(), Moment::from_nanos(0))?
        .ok_or("Panel owner was not admitted")?;
    panel.receive(PanelMessage::Ready(session.clone()), Moment::from_nanos(1))?;
    panel.receive(PanelMessage::Ready(session.clone()), Moment::from_nanos(2))?;
    let configuration = panel.take_command().ok_or("Configuration missing")?;
    assert!(matches!(configuration.message(), PanelMessage::Configure { .. }));
    assert!(panel.take_command().is_none());
    drop(configuration);
    panel.receive(PanelMessage::Applied(session.clone()), Moment::from_nanos(3))?;
    panel.receive(PanelMessage::Applied(session), Moment::from_nanos(4))?;
    let ping = panel.take_command().ok_or("Ping missing")?;
    assert!(matches!(ping.message(), PanelMessage::Ping(_)));
    assert!(panel.take_command().is_none());
    drop(panel);
    drop(lease);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    drop(ping);
    assert_eq!(released.load(Ordering::SeqCst), reserved);
    Ok(())
}

#[test]
fn insufficient_command_admission_does_not_advance_panel_policy() -> Result<()> {
    let lease = MemoryLease::new(
        OWNER_BYTES + PanelCommand::STORAGE_BYTES - 1,
        Box::new(Release(Arc::default())),
    );
    let identity = identity()?;
    let mut panel = PanelOwner::new(identity, lease, Moment::from_nanos(0))?
        .ok_or("Panel owner was not admitted")?;
    panel.receive(PanelMessage::Ready(PanelSession::for_main(identity)?), Moment::from_nanos(1))?;
    assert!(panel.take_command().is_none());
    assert_eq!(panel.deadline(), Some(Moment::from_nanos(30_000_000_000)));
    Ok(())
}

#[test]
fn unavailable_owner_admission_is_a_recoverable_rejection() -> Result<()> {
    let lease = MemoryLease::new(OWNER_BYTES - 1, Box::new(Release(Arc::default())));
    assert!(PanelOwner::new(identity()?, lease, Moment::from_nanos(0))?.is_none());
    Ok(())
}

#[test]
fn delayed_applied_message_retries_without_requiring_the_page_to_repeat_it() -> Result<()> {
    let lease = MemoryLease::new(
        OWNER_BYTES + PanelCommand::STORAGE_BYTES,
        Box::new(Release(Arc::default())),
    );
    let identity = identity()?;
    let session = PanelSession::for_main(identity)?;
    let mut panel = PanelOwner::new(identity, lease, Moment::from_nanos(0))?
        .ok_or("Panel owner was not admitted")?;
    panel.receive(PanelMessage::Ready(session.clone()), Moment::from_nanos(1))?;
    let held = panel.take_command().ok_or("Configuration missing")?;
    panel.receive(PanelMessage::Applied(session.clone()), Moment::from_nanos(2))?;
    assert!(panel.pending_applied);
    assert_eq!(panel.deadline(), Some(Moment::from_nanos(100_000_002)));
    panel.receive(PanelMessage::Pong { session, connected: true }, Moment::from_nanos(3))?;
    assert_eq!(panel.deadline(), Some(Moment::from_nanos(100_000_002)));
    drop(held);
    panel.tick(Moment::from_nanos(100_000_002))?;
    assert!(!panel.pending_applied);
    assert!(matches!(panel.take_command().ok_or("Ping missing")?.message(), PanelMessage::Ping(_)));
    Ok(())
}

#[test]
fn pong_needs_no_command_capacity_and_overloaded_timers_keep_the_original_cutoff() -> Result<()> {
    let reserved = OWNER_BYTES + PanelCommand::STORAGE_BYTES;
    let lease = MemoryLease::new(reserved, Box::new(Release(Arc::default())));
    let identity = identity()?;
    let session = PanelSession::for_main(identity)?;
    let mut panel = PanelOwner::new(identity, lease.clone(), Moment::from_nanos(0))?
        .ok_or("Panel owner was not admitted")?;
    panel.receive(PanelMessage::Ready(session.clone()), Moment::from_nanos(1))?;
    drop(panel.take_command().ok_or("Configuration missing")?);
    panel.receive(PanelMessage::Applied(session.clone()), Moment::from_nanos(2))?;
    panel.receive(PanelMessage::Pong { session, connected: true }, Moment::from_nanos(3))?;
    assert_eq!(panel.policy.state(), PanelState::Connected);
    drop(panel);

    let mut waiting = PanelOwner::new(identity, lease.clone(), Moment::from_nanos(0))?
        .ok_or("Panel owner was not admitted")?;
    let session = PanelSession::for_main(identity)?;
    waiting.receive(PanelMessage::Ready(session.clone()), Moment::from_nanos(1))?;
    let occupied = waiting.take_command().ok_or("Configuration missing")?;
    waiting.receive(PanelMessage::Applied(session), Moment::from_nanos(2))?;
    assert!(waiting.pending_applied);
    waiting.tick(Moment::from_nanos(30_000_000_000))?;
    assert_eq!(waiting.policy.state(), PanelState::Failed);
    assert!(!waiting.pending_applied);
    assert_eq!(waiting.deadline(), None);
    drop(occupied);
    Ok(())
}

#[test]
fn reply_expiration_is_observed_even_when_heartbeat_admission_remains_full() -> Result<()> {
    let lease = MemoryLease::new(
        OWNER_BYTES + PanelCommand::STORAGE_BYTES,
        Box::new(Release(Arc::default())),
    );
    let identity = identity()?;
    let session = PanelSession::for_main(identity)?;
    let mut panel = PanelOwner::new(identity, lease, Moment::from_nanos(0))?
        .ok_or("Panel owner was not admitted")?;
    panel.receive(PanelMessage::Ready(session.clone()), Moment::from_nanos(1))?;
    drop(panel.take_command().ok_or("Configuration missing")?);
    panel.receive(PanelMessage::Applied(session.clone()), Moment::from_nanos(2))?;
    let held = panel.take_command().ok_or("Ping missing")?;
    panel.receive(PanelMessage::Pong { session, connected: true }, Moment::from_nanos(3))?;

    panel.tick(Moment::from_nanos(14_950_000_003))?;
    assert_eq!(panel.policy.state(), PanelState::Connected);
    assert_eq!(panel.deadline(), Some(Moment::from_nanos(15_000_000_003)));
    panel.tick(Moment::from_nanos(15_000_000_003))?;
    assert_eq!(panel.policy.state(), PanelState::Disconnected);
    assert_eq!(panel.deadline(), Some(Moment::from_nanos(15_100_000_003)));
    drop(held);
    panel.tick(Moment::from_nanos(15_100_000_003))?;
    assert!(matches!(
        panel.take_command().ok_or("Retried ping missing")?.message(),
        PanelMessage::Ping(_)
    ));
    Ok(())
}

#[test]
fn handshake_expiration_retires_its_undelivered_configuration() -> Result<()> {
    let lease = MemoryLease::new(
        OWNER_BYTES + PanelCommand::STORAGE_BYTES,
        Box::new(Release(Arc::default())),
    );
    let identity = identity()?;
    let mut panel = PanelOwner::new(identity, lease, Moment::from_nanos(0))?
        .ok_or("Panel owner was not admitted")?;
    panel.receive(PanelMessage::Ready(PanelSession::for_main(identity)?), Moment::from_nanos(1))?;
    assert!(panel.configuration.is_some());
    panel.tick(Moment::from_nanos(30_000_000_000))?;
    assert_eq!(panel.policy.state(), PanelState::Failed);
    assert!(panel.take_command().is_none());
    assert_eq!(panel.deadline(), None);
    Ok(())
}
