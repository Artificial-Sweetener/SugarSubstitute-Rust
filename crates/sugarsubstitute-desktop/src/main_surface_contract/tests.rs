// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify navigation-qualified reply allocation custody.

use super::*;
use sugarsubstitute_contracts::{
    canvas_surface::CanvasViewportMode,
    execution::{Generation, MemoryRelease},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    startup::LaunchId,
};

struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}

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

fn request(
    identity: MainIdentity,
    viewport: bool,
) -> Result<Box<CanvasSurfaceRequest>, Box<dyn Error>> {
    let session = PanelSession::for_main(identity)?;
    let mut value = serde_json::json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":session.as_str(),
        "requestId":7, "type":"surface.prepare", "revision":1,
        "address":{"sessionId":"canvas:w", "viewId":"view:a", "windowId":"window:a", "attachmentId":"attachment:a", "generation":1}
    });
    if viewport {
        value["type"] = serde_json::json!("surface.viewport.request");
        value["command"] = serde_json::json!({"type":"read"});
    }
    let message = CanvasSurfaceMessage::decode(&serde_json::to_vec(&value)?)?;
    let CanvasSurfaceMessage::Request(request) = message else {
        return Err("Missing request".into());
    };
    Ok(request)
}

#[test]
fn response_retains_request_identity_and_returns_its_original_credit() -> Result<(), Box<dyn Error>>
{
    let identity = identity()?;
    let request = request(identity, false)?;
    let lease = MemoryLease::new(SurfaceReply::STORAGE_BYTES, Box::new(Release));
    let reply = SurfaceReply::new(
        identity,
        &request,
        CanvasSurfaceReplyStatus::Ready,
        "",
        lease.claim(SurfaceReply::STORAGE_BYTES)?,
    )?;
    assert_eq!(reply.request_id(), 7);
    assert!(lease.claim(1).is_err());
    let claim = reply.into_claim();
    assert!(lease.claim(1).is_err());
    drop(claim);
    assert!(lease.claim(SurfaceReply::STORAGE_BYTES).is_ok());
    Ok(())
}

#[test]
fn viewport_response_holds_the_original_credit_until_native_delivery_finishes()
-> Result<(), Box<dyn Error>> {
    let identity = identity()?;
    let request = request(identity, true)?;
    let state = CanvasViewportState::new(CanvasViewportMode::Native, 1.0, 320, 240)?;
    let lease = MemoryLease::new(SurfaceReply::STORAGE_BYTES, Box::new(Release));
    let reply = SurfaceReply::viewport(
        identity,
        &request,
        &state,
        "",
        lease.claim(SurfaceReply::STORAGE_BYTES)?,
    )?;
    assert_eq!(reply.identity(), identity);
    assert_eq!(reply.request_id(), request.request_id());
    let payload: serde_json::Value = serde_json::from_slice(reply.payload())?;
    assert_eq!(payload["type"], "surface.viewport.reply");
    assert_eq!(payload["session"], request.session().as_str());
    assert!(lease.claim(1).is_err());
    let claim = reply.into_claim();
    assert_eq!(claim.bytes(), SurfaceReply::STORAGE_BYTES);
    assert!(lease.claim(1).is_err());
    drop(claim);
    assert!(lease.claim(SurfaceReply::STORAGE_BYTES).is_ok());
    Ok(())
}

#[test]
fn viewport_response_rejects_foreign_navigation_and_unadmitted_storage()
-> Result<(), Box<dyn Error>> {
    let identity = identity()?;
    let viewport_request = request(identity, true)?;
    let state = CanvasViewportState::new(CanvasViewportMode::Fit, 0.5, 640, 480)?;
    let lease = MemoryLease::new(SurfaceReply::STORAGE_BYTES, Box::new(Release));
    assert!(matches!(
        SurfaceReply::viewport(identity, &viewport_request, &state, "", lease.claim(1)?),
        Err(SurfaceReplyError::Storage)
    ));
    let foreign = MainIdentity { navigation: Generation::new(2)?, ..identity };
    assert!(matches!(
        SurfaceReply::viewport(
            foreign,
            &viewport_request,
            &state,
            "",
            lease.claim(SurfaceReply::STORAGE_BYTES)?
        ),
        Err(SurfaceReplyError::ForeignNavigation)
    ));
    let prepare = request(identity, false)?;
    assert!(matches!(
        SurfaceReply::viewport(
            identity,
            &prepare,
            &state,
            "",
            lease.claim(SurfaceReply::STORAGE_BYTES)?
        ),
        Err(SurfaceReplyError::Encode(_))
    ));
    assert!(lease.claim(SurfaceReply::STORAGE_BYTES).is_ok());
    Ok(())
}

#[test]
fn watch_reply_uses_its_original_request_claim_and_rejects_foreign_navigation()
-> Result<(), Box<dyn Error>> {
    let identity = identity()?;
    let session = PanelSession::for_main(identity)?;
    let message = CanvasSurfaceMessage::decode(&serde_json::to_vec(&serde_json::json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":session.as_str(),
        "requestId":7, "type":"surface.presentation.request", "schema":1, "enabled":true,
    }))?)?;
    let CanvasSurfaceMessage::Presentation(request) = message else {
        return Err("Missing watch request".into());
    };
    let lease = MemoryLease::new(SurfaceReply::STORAGE_BYTES, Box::new(Release));
    let reply = SurfaceReply::presentation(
        identity,
        &request,
        CanvasSurfaceDiscoveryStatus::Available,
        true,
        "",
        lease.claim(SurfaceReply::STORAGE_BYTES)?,
    )?;
    let payload: serde_json::Value = serde_json::from_slice(reply.payload())?;
    assert_eq!(payload["type"], "surface.presentation.reply");
    assert_eq!(payload["enabled"], true);
    assert_eq!(reply.request_id(), 7);
    assert!(lease.claim(1).is_err());
    drop(reply.into_claim());
    let foreign = MainIdentity { navigation: Generation::new(2)?, ..identity };
    assert!(matches!(
        SurfaceReply::presentation(
            foreign,
            &request,
            CanvasSurfaceDiscoveryStatus::Available,
            true,
            "",
            lease.claim(SurfaceReply::STORAGE_BYTES)?
        ),
        Err(SurfaceReplyError::ForeignNavigation)
    ));
    assert!(matches!(
        SurfaceReply::presentation(
            identity,
            &request,
            CanvasSurfaceDiscoveryStatus::Available,
            true,
            "",
            lease.claim(1)?
        ),
        Err(SurfaceReplyError::Storage)
    ));
    assert!(lease.claim(SurfaceReply::STORAGE_BYTES).is_ok());
    Ok(())
}

#[test]
fn asynchronous_snapshot_owns_independent_storage_and_derives_its_exact_main_session()
-> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::canvas_surface::{
        CanvasSurfaceAddress, CanvasSurfacePresentationChange, CanvasSurfacePresentationDisplay,
        CanvasSurfacePresentationStatus,
    };
    let identity = identity()?;
    let lease = MemoryLease::new(SurfacePresentation::STORAGE_BYTES, Box::new(Release));
    let address = CanvasSurfaceAddress::decode(br#"{"sessionId":"canvas:w","viewId":"view:a","windowId":"window:a","attachmentId":"attachment:a","generation":1}"#)?;
    let change = CanvasSurfacePresentationChange::new(
        address.clone(),
        9,
        "preview:one".into(),
        1,
        CanvasSurfacePresentationStatus::Failed,
        CanvasSurfacePresentationDisplay::Empty,
        "".into(),
    )?;
    assert!(matches!(
        SurfacePresentation::new(identity, &change, lease.claim(1)?),
        Err(SurfaceReplyError::Storage)
    ));
    let notification = SurfacePresentation::new(
        identity,
        &change,
        lease.claim(SurfacePresentation::STORAGE_BYTES)?,
    )?;
    assert_eq!(notification.identity(), identity);
    assert_eq!(notification.sequence(), 9);
    assert_eq!(notification.address(), &address);
    let payload: serde_json::Value = serde_json::from_slice(notification.payload())?;
    assert_eq!(payload["session"], PanelSession::for_main(identity)?.as_str());
    assert_eq!(payload["requestId"], 0);
    assert_eq!(payload["type"], "surface.presentation.changed");
    assert_eq!(payload["change"]["sequence"], 9);
    assert!(lease.claim(1).is_err());
    let claim = notification.into_claim();
    assert!(lease.claim(1).is_err());
    drop(claim);
    assert!(lease.claim(SurfacePresentation::STORAGE_BYTES).is_ok());
    Ok(())
}
