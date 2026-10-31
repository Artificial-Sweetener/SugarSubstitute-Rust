// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove single-lane fallback delivery, navigation fencing and original ingress custody.

use super::*;
#[path = "tests/lanes.rs"]
mod lanes;
#[path = "tests/presentation.rs"]
mod presentation;
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
        run: crate::production::child::test_support::run()?,
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
fn handshake_failure_preserves_an_already_queued_surface_refusal() -> Result<()> {
    let reserved = OWNER_BYTES + SurfaceReply::STORAGE_BYTES;
    let lease = MemoryLease::new(reserved, Box::new(Release(Arc::default())));
    let identity = identity()?;
    let session = PanelSession::for_main(identity)?;
    let mut deliveries = Deliveries::new();
    deliveries.bind(identity, &lease)?;
    deliveries.surface(
        identity,
        surface_request(&session, 23)?,
        &lease,
        lease.claim(SurfaceReply::STORAGE_BYTES)?,
    )?;
    let mut panel = PanelOwner::new(identity, lease.clone(), Moment::from_nanos(0))?
        .ok_or("Panel owner was not admitted")?;
    panel.tick(Moment::from_nanos(30_000_000_000))?;
    assert_eq!(panel.policy.state(), PanelState::Failed);
    let Some(DesktopStartupCommand::SendSurface(reply)) = deliveries.take_command(identity) else {
        return Err("Policy failure discarded an admitted refusal".into());
    };
    let value: serde_json::Value = serde_json::from_slice(reply.payload())?;
    assert_eq!(value["requestId"], 23);
    assert_eq!(value["status"], "unavailable");
    Ok(())
}

#[test]
fn limited_panel_discovery_is_explicit_and_keeps_native_custody() -> Result<()> {
    let released = Arc::new(AtomicU64::new(0));
    let reserved = OWNER_BYTES + SurfaceReply::STORAGE_BYTES;
    let lease = MemoryLease::new(reserved, Box::new(Release(released.clone())));
    let identity = identity()?;
    let session = PanelSession::for_main(identity)?;
    let mut deliveries = Deliveries::new();
    deliveries.bind(identity, &lease)?;
    let message = CanvasSurfaceMessage::decode(&serde_json::to_vec(&serde_json::json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":session.as_str(),
        "requestId":17, "type":"surface.capabilities.request", "schema":1
    }))?)?;
    deliveries.surface(
        identity,
        message.clone(),
        &lease,
        lease.claim(SurfaceReply::STORAGE_BYTES)?,
    )?;
    let Some(DesktopStartupCommand::SendSurface(reply)) = deliveries.take_command(identity) else {
        return Err("Surface discovery response missing".into());
    };
    let value: serde_json::Value = serde_json::from_slice(reply.payload())?;
    assert_eq!(value["type"], "surface.capabilities.reply");
    assert_eq!(value["requestId"], 17);
    assert_eq!(value["session"], session.as_str());
    assert_eq!(value["status"], "unsupported");
    assert_eq!(
        value["capabilities"],
        serde_json::json!({
            "schema":1, "imageFormats":[], "viewportCommands":[], "viewportState":false,
            "previewFormats":[], "comparison":false, "overview":false
        })
    );
    let other = MemoryLease::new(reserved, Box::new(Release(Arc::default())));
    assert!(
        deliveries
            .surface(identity, message.clone(), &other, other.claim(SurfaceReply::STORAGE_BYTES)?)
            .is_err()
    );
    let foreign = MainIdentity { navigation: Generation::new(2)?, ..identity };
    assert!(
        deliveries
            .surface(foreign, message, &lease, other.claim(SurfaceReply::STORAGE_BYTES)?)
            .is_err()
    );
    assert!(deliveries.take_command(identity).is_none());
    drop(deliveries);
    drop(lease);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    drop(reply);
    assert_eq!(released.load(Ordering::SeqCst), reserved);
    Ok(())
}

#[test]
fn discovery_replies_reject_foreign_sessions_and_inadequate_delivery_credit() -> Result<()> {
    let identity = identity()?;
    let foreign = MainIdentity { navigation: Generation::new(2)?, ..identity };
    for (request_identity, bytes) in
        [(foreign, SurfaceReply::STORAGE_BYTES), (identity, SurfaceReply::STORAGE_BYTES - 1)]
    {
        let released = Arc::new(AtomicU64::new(0));
        let lease = MemoryLease::new(bytes, Box::new(Release(released.clone())));
        let session = PanelSession::for_main(request_identity)?;
        let message = CanvasSurfaceMessage::decode(&serde_json::to_vec(&serde_json::json!({
            "protocol":"sugarcubes.canvas", "version":3, "session":session.as_str(),
            "requestId":17, "type":"surface.capabilities.request", "schema":1
        }))?)?;
        let CanvasSurfaceMessage::Discovery(request) = message else {
            return Err("Expected discovery request".into());
        };
        let result = SurfaceReply::discovery(
            identity,
            &request,
            CanvasSurfaceDiscoveryStatus::Unsupported,
            &CanvasSurfaceCapabilities::default(),
            "",
            lease.claim(bytes)?,
        );
        if request_identity == foreign {
            assert!(matches!(
                result,
                Err(sugarsubstitute_desktop::SurfaceReplyError::ForeignNavigation)
            ));
        } else {
            assert!(matches!(result, Err(sugarsubstitute_desktop::SurfaceReplyError::Storage)));
        }
        assert!(lease.claim(bytes).is_ok());
        drop(lease);
        assert_eq!(released.load(Ordering::SeqCst), bytes);
    }
    Ok(())
}

#[test]
fn surface_negotiation_reports_unavailable_and_retains_the_ingress_claim() -> Result<()> {
    let released = Arc::new(AtomicU64::new(0));
    let reserved = OWNER_BYTES + SurfaceReply::STORAGE_BYTES;
    let lease = MemoryLease::new(reserved, Box::new(Release(released.clone())));
    let identity = identity()?;
    let session = PanelSession::for_main(identity)?;
    let mut deliveries = Deliveries::new();
    deliveries.bind(identity, &lease)?;
    let claim = lease.claim(SurfaceReply::STORAGE_BYTES)?;
    deliveries.surface(identity, surface_request(&session, 7)?, &lease, claim)?;
    let Some(DesktopStartupCommand::SendSurface(reply)) = deliveries.take_command(identity) else {
        return Err("Surface reply missing".into());
    };
    let payload: serde_json::Value = serde_json::from_slice(reply.payload())?;
    assert_eq!(payload["status"], "unavailable");
    assert_eq!(payload["requestId"], 7);
    assert_eq!(payload["revision"], 3);
    assert_eq!(payload["address"]["viewId"], "view:one");
    drop(deliveries);
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
    let mut deliveries = Deliveries::new();
    deliveries.bind(identity, &lease)?;
    let foreign = MainIdentity { navigation: Generation::new(2)?, ..identity };
    assert!(
        deliveries
            .surface(
                foreign,
                surface_request(&session, 1)?,
                &lease,
                lease.claim(SurfaceReply::STORAGE_BYTES)?,
            )
            .is_err()
    );
    assert!(
        deliveries
            .surface(
                identity,
                surface_request(&session, 2)?,
                &other,
                other.claim(SurfaceReply::STORAGE_BYTES)?,
            )
            .is_err()
    );
    let foreign_session = PanelSession::for_main(foreign)?;
    assert!(
        deliveries
            .surface(
                identity,
                surface_request(&foreign_session, 3)?,
                &lease,
                lease.claim(SurfaceReply::STORAGE_BYTES)?,
            )
            .is_err()
    );
    assert!(deliveries.take_command(identity).is_none());
    Ok(())
}

#[test]
fn canvas_context_reports_only_the_currently_implemented_display_capabilities() -> Result<()> {
    let released = Arc::new(AtomicU64::new(0));
    let reserved = OWNER_BYTES + CanvasContextReply::STORAGE_BYTES;
    let lease = MemoryLease::new(reserved, Box::new(Release(released.clone())));
    let identity = identity()?;
    let session = PanelSession::for_main(identity)?;
    let mut deliveries = Deliveries::new();
    deliveries.bind(identity, &lease)?;
    let request = context_request(&session)?;
    deliveries.context(
        identity,
        request,
        &lease,
        lease.claim(CanvasContextReply::STORAGE_BYTES)?,
    )?;
    let Some(DesktopStartupCommand::SendCanvasContext(reply)) = deliveries.take_command(identity)
    else {
        return Err("Canvas context reply missing".into());
    };
    let value: serde_json::Value = serde_json::from_slice(reply.payload())?;
    assert_eq!(value["requestId"], 9);
    assert_eq!(value["session"], session.as_str());
    assert_eq!(value["context"]["canvas"]["capabilities"]["displayOutput"], false);
    assert_eq!(value["context"]["canvas"]["capabilities"]["insertImageLayer"], false);
    assert_eq!(value["context"]["document"], serde_json::Value::Null);
    drop(deliveries);
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
    let mut deliveries = Deliveries::new();
    deliveries.bind(identity, &lease)?;
    let foreign = MainIdentity { navigation: Generation::new(2)?, ..identity };
    assert!(
        deliveries
            .context(
                foreign,
                context_request(&session)?,
                &lease,
                lease.claim(CanvasContextReply::STORAGE_BYTES)?,
            )
            .is_err()
    );
    assert!(
        deliveries
            .context(
                identity,
                context_request(&session)?,
                &other,
                other.claim(CanvasContextReply::STORAGE_BYTES)?,
            )
            .is_err()
    );
    assert!(
        deliveries
            .context(
                identity,
                context_request(&PanelSession::for_main(foreign)?)?,
                &lease,
                lease.claim(CanvasContextReply::STORAGE_BYTES)?,
            )
            .is_err()
    );
    assert!(deliveries.take_command(identity).is_none());
    Ok(())
}
