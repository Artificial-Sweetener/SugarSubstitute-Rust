// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject impossible duplicate lane admission and preserve fairness and original native claims.

use super::*;

fn cancellation(session: &PanelSession, request_id: u64) -> Result<CanvasSurfaceMessage> {
    Ok(CanvasSurfaceMessage::decode(&serde_json::to_vec(&serde_json::json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":session.as_str(),
        "requestId":request_id, "type":"cancel"
    }))?)?)
}

#[test]
fn ordinary_control_and_cancellation_have_independent_slots_and_bounded_service() -> Result<()> {
    let bytes = SurfaceReply::STORAGE_BYTES;
    let lease = MemoryLease::new(5 * bytes, Box::new(Release(Arc::default())));
    let identity = identity()?;
    let session = PanelSession::for_main(identity)?;
    let mut deliveries = Deliveries::new();
    deliveries.surface(
        identity,
        surface_request(&session, 1)?,
        &lease,
        lease.claim(bytes)?,
        MainHosting::Windowed,
    )?;
    deliveries.complete_control(PanelIngressReceipt::control(identity, lease.claim(bytes)?)?)?;
    deliveries.surface(
        identity,
        cancellation(&session, 1)?,
        &lease,
        lease.claim(bytes)?,
        MainHosting::Windowed,
    )?;

    assert!(
        deliveries
            .native(
                identity,
                &context_request(&session)?,
                &lease,
                lease.claim(bytes)?,
                &unavailable(),
            )
            .is_err()
    );
    assert!(
        deliveries
            .complete_control(PanelIngressReceipt::control(identity, lease.claim(bytes)?)?)
            .is_err()
    );
    assert!(
        deliveries
            .surface(
                identity,
                cancellation(&session, 2)?,
                &lease,
                lease.claim(bytes)?,
                MainHosting::Windowed
            )
            .is_err()
    );
    let Some(DesktopStartupCommand::SendSurface(reply)) = deliveries.take_command(identity) else {
        return Err("Ordinary refusal lost its service turn".into());
    };
    let value: serde_json::Value = serde_json::from_slice(reply.payload())?;
    assert_eq!(value["requestId"], 1);
    drop(reply);
    deliveries.surface(
        identity,
        surface_request(&session, 2)?,
        &lease,
        lease.claim(bytes)?,
        MainHosting::Windowed,
    )?;
    assert!(matches!(
        deliveries.take_command(identity),
        Some(DesktopStartupCommand::CompletePanelIngress(_))
    ));
    assert!(deliveries.control.is_none());
    deliveries.complete_control(PanelIngressReceipt::control(identity, lease.claim(bytes)?)?)?;
    assert!(matches!(
        deliveries.take_command(identity),
        Some(DesktopStartupCommand::CompletePanelIngress(_))
    ));
    assert!(deliveries.cancellation.is_none());
    let Some(DesktopStartupCommand::SendSurface(reply)) = deliveries.take_command(identity) else {
        return Err("Replenished ordinary refusal was starved".into());
    };
    let value: serde_json::Value = serde_json::from_slice(reply.payload())?;
    assert_eq!(value["requestId"], 2);
    drop(reply);
    drop(deliveries.take_command(identity));
    assert!(deliveries.take_command(identity).is_none());
    assert!(lease.claim(5 * bytes).is_ok());
    Ok(())
}

#[test]
fn receipt_retains_its_original_claim_until_the_native_command_retires() -> Result<()> {
    let bytes = PanelIngressReceipt::STORAGE_BYTES;
    let released = Arc::new(AtomicU64::new(0));
    let lease = MemoryLease::new(bytes, Box::new(Release(released.clone())));
    let identity = identity()?;
    let mut deliveries = Deliveries::new();
    deliveries.bind(identity, &lease)?;
    deliveries.complete_control(PanelIngressReceipt::control(identity, lease.claim(bytes)?)?)?;
    assert!(lease.claim(1).is_err());
    let command = deliveries.take_command(identity).ok_or("Control receipt missing")?;
    deliveries.clear();
    drop(lease);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    drop(command);
    assert_eq!(released.load(Ordering::SeqCst), bytes);
    Ok(())
}

#[test]
fn clear_releases_every_lane_and_rebinding_requires_the_explicit_fence() -> Result<()> {
    let bytes = SurfaceReply::STORAGE_BYTES;
    let released = Arc::new(AtomicU64::new(0));
    let lease = MemoryLease::new(3 * bytes, Box::new(Release(released.clone())));
    let identity = identity()?;
    let session = PanelSession::for_main(identity)?;
    let mut deliveries = Deliveries::new();
    deliveries.native(
        identity,
        &context_request(&session)?,
        &lease,
        lease.claim(bytes)?,
        &unavailable(),
    )?;
    deliveries.complete_control(PanelIngressReceipt::control(identity, lease.claim(bytes)?)?)?;
    deliveries.surface(
        identity,
        cancellation(&session, 9)?,
        &lease,
        lease.claim(bytes)?,
        MainHosting::Windowed,
    )?;
    let next = MainIdentity { navigation: Generation::new(2)?, ..identity };
    let other = MemoryLease::new(bytes, Box::new(Release(Arc::default())));
    assert!(deliveries.bind(next, &other).is_err());
    assert!(deliveries.bind(identity, &other).is_err());
    drop(lease);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    deliveries.clear();
    assert_eq!(released.load(Ordering::SeqCst), 3 * bytes);
    deliveries.bind(next, &other)?;
    assert!(deliveries.take_command(next).is_none());
    Ok(())
}

#[test]
fn a_foreign_current_navigation_cannot_receive_predecessor_replies_or_receipts() -> Result<()> {
    let bytes = SurfaceReply::STORAGE_BYTES;
    let lease = MemoryLease::new(3 * bytes, Box::new(Release(Arc::default())));
    let identity = identity()?;
    let session = PanelSession::for_main(identity)?;
    let mut deliveries = Deliveries::new();
    deliveries.surface(
        identity,
        surface_request(&session, 1)?,
        &lease,
        lease.claim(bytes)?,
        MainHosting::Windowed,
    )?;
    deliveries.complete_control(PanelIngressReceipt::control(identity, lease.claim(bytes)?)?)?;
    deliveries.surface(
        identity,
        cancellation(&session, 1)?,
        &lease,
        lease.claim(bytes)?,
        MainHosting::Windowed,
    )?;
    let next = MainIdentity { navigation: Generation::new(2)?, ..identity };
    assert!(deliveries.take_command(next).is_none());
    assert!(deliveries.take_command(identity).is_none());
    assert!(lease.claim(3 * bytes).is_ok());
    Ok(())
}

#[test]
fn a_foreign_control_receipt_cannot_fill_the_current_navigation_slot() -> Result<()> {
    let bytes = PanelIngressReceipt::STORAGE_BYTES;
    let lease = MemoryLease::new(bytes, Box::new(Release(Arc::default())));
    let identity = identity()?;
    let foreign = MainIdentity { navigation: Generation::new(2)?, ..identity };
    let mut deliveries = Deliveries::new();
    deliveries.bind(identity, &lease)?;
    assert!(
        deliveries
            .complete_control(PanelIngressReceipt::control(foreign, lease.claim(bytes)?)?)
            .is_err()
    );
    assert!(deliveries.take_command(identity).is_none());
    deliveries.complete_control(PanelIngressReceipt::control(identity, lease.claim(bytes)?)?)?;
    let Some(DesktopStartupCommand::CompletePanelIngress(receipt)) =
        deliveries.take_command(identity)
    else {
        return Err("Current receipt was lost after rejecting a foreign one".into());
    };
    assert_eq!(receipt.identity(), identity);
    Ok(())
}
