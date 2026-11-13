// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Limited hosts refuse presentation watches while preserving original reply custody.

use super::*;

#[test]
fn limited_presentation_observation_is_explicit_and_keeps_ingress_credit() -> Result<()> {
    let released = Arc::new(AtomicU64::new(0));
    let reserved = OWNER_BYTES + SurfaceReply::STORAGE_BYTES;
    let lease = MemoryLease::new(reserved, Box::new(Release(released.clone())));
    let identity = identity()?;
    let session = PanelSession::for_main(identity)?;
    let mut deliveries = Deliveries::new();
    for enabled in [true, false] {
        let message = CanvasSurfaceMessage::decode(&serde_json::to_vec(&serde_json::json!({
            "protocol":"sugarcubes.canvas", "version":3, "session":session.as_str(),
            "requestId":17, "type":"surface.presentation.request", "schema":1, "enabled":enabled
        }))?)?;
        deliveries.surface(
            identity,
            message,
            &lease,
            lease.claim(SurfaceReply::STORAGE_BYTES)?,
            MainHosting::Windowed,
        )?;
        let Some(DesktopStartupCommand::SendSurface(reply)) = deliveries.take_command(identity)
        else {
            return Err("Presentation watch refusal missing".into());
        };
        let value: serde_json::Value = serde_json::from_slice(reply.payload())?;
        assert_eq!(value["type"], "surface.presentation.reply");
        assert_eq!(value["requestId"], 17);
        assert_eq!(value["session"], session.as_str());
        assert_eq!(value["status"], "unsupported");
        assert_eq!(value["enabled"], false);
        assert!(lease.claim(SurfaceReply::STORAGE_BYTES).is_err());
        drop(reply);
        assert!(lease.claim(SurfaceReply::STORAGE_BYTES).is_ok());
    }
    assert!(deliveries.take_command(identity).is_none());
    drop(deliveries);
    drop(lease);
    assert_eq!(released.load(Ordering::SeqCst), reserved);
    Ok(())
}

#[test]
fn a_composition_hosted_page_hears_its_surface_is_unavailable_until_it_can_answer() -> Result<()> {
    let reserved = OWNER_BYTES + SurfaceReply::STORAGE_BYTES;
    let lease = MemoryLease::new(reserved, Box::new(Release(Arc::default())));
    let identity = identity()?;
    let session = PanelSession::for_main(identity)?;
    let mut deliveries = Deliveries::new();
    for (hosting, status) in
        [(MainHosting::Composition, "unavailable"), (MainHosting::Windowed, "unsupported")]
    {
        for (request, reply) in [
            (
                serde_json::json!({"type": "surface.capabilities.request", "schema": 3}),
                "surface.capabilities.reply",
            ),
            (
                serde_json::json!({"type": "surface.presentation.request", "schema": 3, "enabled": true}),
                "surface.presentation.reply",
            ),
        ] {
            let mut value = serde_json::json!({"protocol": "sugarcubes.canvas", "version": 3,
                "session": session.as_str(), "requestId": 5});
            value
                .as_object_mut()
                .ok_or("Envelope")?
                .extend(request.as_object().ok_or("Request")?.clone());
            let message = CanvasSurfaceMessage::decode(&serde_json::to_vec(&value)?)?;
            let claim = lease.claim(SurfaceReply::STORAGE_BYTES)?;
            deliveries.surface(identity, message, &lease, claim, hosting)?;
            let Some(DesktopStartupCommand::SendSurface(answer)) =
                deliveries.take_command(identity)
            else {
                return Err("Surface answer missing".into());
            };
            let answered: serde_json::Value = serde_json::from_slice(answer.payload())?;
            assert_eq!(answered["type"], reply);
            assert_eq!(answered["status"], status, "{hosting:?} {reply}");
        }
    }
    Ok(())
}
