// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise original-credit transfer, independent cancellation and bounded refusal.

use super::*;
use crate::PanelIngressReceipt;
use std::error::Error;
use sugarsubstitute_contracts::{
    execution::{Generation, MemoryRelease},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    startup::{LaunchId, MainIdentity},
};

struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}

/// Qualify custody without native handles or scheduler substitutes.
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

/// Encode a real ordinary context reply using its original transferred claim.
fn ordinary_reply(
    request_id: u64,
    claim: sugarsubstitute_contracts::execution::MemoryClaim,
) -> Result<NativeDelivery, Box<dyn Error>> {
    use sugarsubstitute_contracts::{
        canvas_context::{CanvasContextRequest, CanvasDisplayCapability},
        panel::PanelSession,
    };
    let identity = identity()?;
    let session = PanelSession::for_main(identity)?;
    let request = CanvasContextRequest::decode(&serde_json::to_vec(&serde_json::json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":session.as_str(),
        "requestId":request_id, "type":"context.request"
    }))?)?;
    Ok(NativeDelivery::Context(crate::CanvasContextReply::new(
        identity,
        &request,
        CanvasDisplayCapability::Available,
        claim,
    )?))
}

#[test]
fn issued_permits_reserve_all_credit_and_cannot_be_replayed() -> Result<(), Box<dyn Error>> {
    let lease = MemoryLease::new(4 * STORAGE_BYTES, Box::new(Release));
    let mut owner = Admission::new(&lease)?;
    assert!(lease.claim(1).is_err());
    owner.synchronize(Some(17));
    let mut grants = Vec::new();
    for lane in LANES {
        let action = owner.action()?.ok_or("Missing grant")?;
        assert_eq!(action.lane, lane);
        assert_eq!(action.kind, TransportActionKind::Grant);
        grants.push(action);
    }
    assert!(owner.action()?.is_none());
    let ordinary = grants[1];
    assert!(!owner.issued(18, ordinary.lane, ordinary.ticket));
    let claim = owner
        .consume(17, ordinary.lane, ordinary.ticket, Some(1))?
        .ok_or("Missing admitted credit")?;
    assert!(owner.consume(17, ordinary.lane, ordinary.ticket, Some(1))?.is_none());
    assert!(owner.action()?.is_none());
    assert!(lease.claim(1).is_err());
    drop(claim);
    drop(owner);
    assert!(lease.claim(4 * STORAGE_BYTES).is_ok());
    Ok(())
}

#[test]
fn control_credit_returns_only_after_typed_delivery_completion_and_never_after_close()
-> Result<(), Box<dyn Error>> {
    let lease = MemoryLease::new(4 * STORAGE_BYTES, Box::new(Release));
    let mut owner = Admission::new(&lease)?;
    owner.synchronize(Some(17));
    let grant = owner.action()?.ok_or("Missing control grant")?;
    owner.action_completed(17, grant)?;
    assert_eq!(owner.action()?.ok_or("Missing ordinary grant")?.lane, Lane::Ordinary);
    assert_eq!(owner.action()?.ok_or("Missing cancellation grant")?.lane, Lane::Cancellation);
    let claim =
        owner.consume(17, grant.lane, grant.ticket, None)?.ok_or("Missing admitted credit")?;
    let delivery = NativeDelivery::Ingress(PanelIngressReceipt::control(identity()?, claim)?);
    let ticket = owner.begin_delivery(&delivery)?;
    assert!(lease.claim(1).is_err());
    assert!(!owner.issued(17, grant.lane, ticket));
    owner.completed(ticket, delivery)?;
    let renewed = owner.action()?.ok_or("Credit was not returned")?;
    assert_eq!(renewed.lane, Lane::Control);
    assert_ne!(renewed.ticket, ticket);
    owner.close();
    assert!(owner.action()?.is_none());
    Ok(())
}

#[test]
fn raw_overflow_refuses_only_unconsumed_tickets_without_losing_application_credit()
-> Result<(), Box<dyn Error>> {
    let lease = MemoryLease::new(4 * STORAGE_BYTES, Box::new(Release));
    let mut owner = Admission::new(&lease)?;
    owner.synchronize(Some(17));
    let control = owner.action()?.ok_or("Missing control grant")?;
    let ordinary = owner.action()?.ok_or("Missing ordinary grant")?;
    let cancel = owner.action()?.ok_or("Missing cancel grant")?;
    for grant in [control, ordinary, cancel] {
        owner.action_completed(17, grant)?;
    }
    let ordinary_claim = owner
        .consume(17, ordinary.lane, ordinary.ticket, Some(7))?
        .ok_or("Missing admitted credit")?;
    owner.overflow();
    let refused_control = owner.action()?.ok_or("Missing refusal")?;
    assert_eq!(refused_control.ticket, control.ticket);
    assert_eq!(refused_control.kind, TransportActionKind::RetryableRefusal);
    assert_eq!(refused_control.code(), "r");
    assert_eq!(owner.slots[Lane::Ordinary.index()].phase, Phase::Application);
    owner.action_completed(17, refused_control)?;
    let refused_cancel = owner.action()?.ok_or("Missing cancel refusal")?;
    assert_eq!(refused_cancel.ticket, cancel.ticket);
    assert_eq!(refused_cancel.kind, TransportActionKind::RetryableRefusal);
    let renewed = owner.action()?.ok_or("Missing renewed control")?;
    assert_eq!(renewed.lane, Lane::Control);
    assert!(lease.claim(1).is_err());
    drop(ordinary_claim);
    Ok(())
}

#[test]
fn cancellation_has_its_own_credit_and_must_match_the_active_ordinary_request()
-> Result<(), Box<dyn Error>> {
    let lease = MemoryLease::new(4 * STORAGE_BYTES, Box::new(Release));
    let mut owner = Admission::new(&lease)?;
    owner.synchronize(Some(17));
    owner.action()?;
    let ordinary = owner.action()?.ok_or("Missing ordinary grant")?;
    let cancel = owner.action()?.ok_or("Missing cancellation grant")?;
    let original = owner
        .consume(17, ordinary.lane, ordinary.ticket, Some(3))?
        .ok_or("Missing admitted credit")?;
    assert!(owner.consume(17, cancel.lane, cancel.ticket, Some(4))?.is_none());
    let cancellation =
        owner.consume(17, cancel.lane, cancel.ticket, Some(3))?.ok_or("Missing admitted credit")?;
    assert_eq!(owner.slots[Lane::Ordinary.index()].phase, Phase::Application);
    owner.synchronize(None);
    assert!(!owner.issued(17, cancel.lane, cancel.ticket));
    assert!(owner.action()?.is_none());
    drop((original, cancellation));
    Ok(())
}

#[test]
fn synchronous_browser_receipt_cannot_reissue_credit_ahead_of_the_original_grant_callback()
-> Result<(), Box<dyn Error>> {
    let lease = MemoryLease::new(4 * STORAGE_BYTES, Box::new(Release));
    let mut owner = Admission::new(&lease)?;
    owner.synchronize(Some(17));
    let grant = owner.action()?.ok_or("Missing control grant")?;
    let claim =
        owner.consume(17, grant.lane, grant.ticket, None)?.ok_or("Missing admitted credit")?;
    let delivery = NativeDelivery::Ingress(PanelIngressReceipt::control(identity()?, claim)?);
    let ticket = owner.begin_delivery(&delivery)?;
    owner.completed(ticket, delivery)?;
    assert!(owner.slots[Lane::Control.index()].grant_pending);
    assert_eq!(owner.action()?.ok_or("Missing ordinary grant")?.lane, Lane::Ordinary);
    assert_eq!(owner.action()?.ok_or("Missing cancellation grant")?.lane, Lane::Cancellation);
    assert!(owner.action()?.is_none());
    owner.action_completed(17, grant)?;
    assert_eq!(owner.action()?.ok_or("Missing renewed control")?.lane, Lane::Control);
    Ok(())
}

#[test]
fn late_cancellation_refusal_preserves_newer_ordinary_credit_and_its_pending_grant()
-> Result<(), Box<dyn Error>> {
    let lease = MemoryLease::new(4 * STORAGE_BYTES, Box::new(Release));
    let mut owner = Admission::new(&lease)?;
    owner.synchronize(Some(17));
    let control = owner.action()?.ok_or("Missing control grant")?;
    let first = owner.action()?.ok_or("Missing ordinary grant")?;
    let cancel = owner.action()?.ok_or("Missing cancellation grant")?;
    for action in [control, first, cancel] {
        owner.action_completed(17, action)?;
    }
    let claim = owner
        .consume(17, Lane::Ordinary, first.ticket, Some(91))?
        .ok_or("Missing admitted credit")?;
    let reply = ordinary_reply(91, claim)?;
    let ticket = owner.begin_delivery(&reply)?;
    owner.completed(ticket, reply)?;
    let next = owner.action()?.ok_or("Missing renewed ordinary grant")?;
    assert_eq!(next.lane, Lane::Ordinary);
    let original =
        owner.consume(17, next.lane, next.ticket, Some(1))?.ok_or("Missing admitted credit")?;
    assert!(owner.consume(17, cancel.lane, cancel.ticket, Some(91))?.is_none());
    owner.reject(cancel.lane, cancel.ticket);
    let refused = owner.action()?.ok_or("Missing late cancellation refusal")?;
    assert_eq!(refused.lane, Lane::Cancellation);
    assert_eq!(refused.kind, TransportActionKind::PermanentRejection);
    assert_eq!(refused.code(), "a");
    owner.action_completed(17, refused)?;
    let renewed = owner.action()?.ok_or("Missing cancellation capacity")?;
    assert_eq!(renewed.lane, Lane::Cancellation);
    let ordinary = &owner.slots[Lane::Ordinary.index()];
    assert_eq!(ordinary.ticket, next.ticket);
    assert_eq!(ordinary.request_id, Some(1));
    assert_eq!(ordinary.phase, Phase::Application);
    assert!(ordinary.grant_pending);
    assert!(ordinary.claim.is_none());
    assert!(lease.claim(1).is_err());
    owner.action_completed(17, next)?;
    assert_eq!(owner.slots[Lane::Ordinary.index()].request_id, Some(1));
    drop(original);
    Ok(())
}

#[test]
fn invalid_control_and_cancellation_consume_their_lane_without_replaying_or_settling_ordinary()
-> Result<(), Box<dyn Error>> {
    use crate::main_panel_ingress::decode_payload;
    use sugarsubstitute_contracts::panel::PanelSession;
    for lane in [Lane::Control, Lane::Cancellation] {
        let lease = MemoryLease::new(4 * STORAGE_BYTES, Box::new(Release));
        let mut owner = Admission::new(&lease)?;
        owner.synchronize(Some(17));
        let control = owner.action()?.ok_or("Missing control grant")?;
        let ordinary = owner.action()?.ok_or("Missing ordinary grant")?;
        let cancel = owner.action()?.ok_or("Missing cancel grant")?;
        for grant in [control, ordinary, cancel] {
            owner.action_completed(17, grant)?;
        }
        let original = owner
            .consume(17, ordinary.lane, ordinary.ticket, Some(7))?
            .ok_or("Missing ordinary claim")?;
        let session = PanelSession::for_main(identity()?)?;
        let (mut payload, rejected) = match lane {
            Lane::Control => (
                serde_json::json!({"protocol":"sugarcubes.panel", "version":3, "session":session.as_str(), "type":"ready", "extra":1}),
                control,
            ),
            Lane::Cancellation => (
                serde_json::json!({"protocol":"sugarcubes.canvas", "version":3, "session":session.as_str(), "type":"cancel", "requestId":7, "extra":1}),
                cancel,
            ),
            Lane::Ordinary => return Err("Expected control lane".into()),
        };
        assert!(decode_payload(&serde_json::to_string(&payload)?, &session).is_none());
        owner.reject(lane, rejected.ticket);
        let rejection = owner.action()?.ok_or("Missing permanent rejection")?;
        assert_eq!(rejection.lane, lane);
        assert_eq!(rejection.kind, TransportActionKind::PermanentRejection);
        assert_eq!(rejection.code(), "a");
        assert!(owner.action()?.is_none());
        owner.action_completed(17, rejection)?;
        let renewed = owner.action()?.ok_or("Missing fresh lane grant")?;
        assert_eq!(renewed.lane, lane);
        assert_ne!(renewed.ticket, rejected.ticket);
        payload
            .as_object_mut()
            .ok_or("Expected object")?
            .remove("extra")
            .ok_or("Missing rejected field")?;
        let valid = decode_payload(&serde_json::to_string(&payload)?, &session)
            .ok_or("Valid message was refused")?;
        let claim = owner
            .consume(17, lane, renewed.ticket, valid.request_id())?
            .ok_or("Missing fresh lane credit")?;
        assert_eq!(owner.slots[Lane::Ordinary.index()].phase, Phase::Application);
        assert_eq!(owner.slots[Lane::Ordinary.index()].request_id, Some(7));
        assert!(owner.slots[Lane::Ordinary.index()].claim.is_none());
        assert!(lease.claim(1).is_err());
        drop((original, claim));
    }
    Ok(())
}

#[test]
fn invalid_ordinary_request_uses_one_terminal_refusal_and_preserves_its_original_credit()
-> Result<(), Box<dyn Error>> {
    let lease = MemoryLease::new(4 * STORAGE_BYTES, Box::new(Release));
    let mut owner = Admission::new(&lease)?;
    owner.synchronize(Some(17));
    let control = owner.action()?.ok_or("Missing control grant")?;
    let ordinary = owner.action()?.ok_or("Missing ordinary grant")?;
    let cancel = owner.action()?.ok_or("Missing cancel grant")?;
    for action in [control, ordinary, cancel] {
        owner.action_completed(17, action)?;
    }
    owner.reject(ordinary.lane, ordinary.ticket);
    let rejected = owner.action()?.ok_or("Missing ordinary rejection")?;
    assert_eq!(rejected.kind, TransportActionKind::PermanentRejection);
    assert_eq!(rejected.code(), "r");
    assert!(owner.slots[Lane::Ordinary.index()].claim.is_some());
    assert!(owner.action()?.is_none());
    owner.action_completed(17, rejected)?;
    let renewed = owner.action()?.ok_or("Missing fresh ordinary grant")?;
    assert_ne!(renewed.ticket, ordinary.ticket);
    assert!(lease.claim(1).is_err());
    Ok(())
}

#[test]
fn asynchronous_presentation_cannot_begin_or_complete_an_ordinary_credit()
-> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::canvas_surface::{
        CanvasSurfaceAddress, CanvasSurfacePresentationChange, CanvasSurfacePresentationDisplay,
        CanvasSurfacePresentationStatus,
    };
    let lease = MemoryLease::new(4 * STORAGE_BYTES, Box::new(Release));
    let notification_lease = MemoryLease::new(STORAGE_BYTES, Box::new(Release));
    let mut owner = Admission::new(&lease)?;
    owner.synchronize(Some(17));
    let mut ordinary = None;
    for _ in LANES {
        let action = owner.action()?.ok_or("Missing grant")?;
        owner.action_completed(17, action)?;
        if action.lane == Lane::Ordinary {
            ordinary = Some(action);
        }
    }
    let ordinary = ordinary.ok_or("Missing ordinary grant")?;
    let claim = owner
        .consume(17, Lane::Ordinary, ordinary.ticket, Some(1))?
        .ok_or("Missing ordinary claim")?;
    let address = CanvasSurfaceAddress::decode(br#"{"sessionId":"canvas:w","viewId":"view:a","windowId":"window:a","attachmentId":"attachment:a","generation":1}"#)?;
    let change = CanvasSurfacePresentationChange::new(
        address,
        1,
        "preview:one".into(),
        1,
        CanvasSurfacePresentationStatus::Failed,
        CanvasSurfacePresentationDisplay::Empty,
        "".into(),
    )?;
    let notification = NativeDelivery::Presentation(crate::SurfacePresentation::new(
        identity()?,
        &change,
        notification_lease.claim(STORAGE_BYTES)?,
    )?);
    assert_eq!(notification.lane(), None);
    assert_eq!(notification.request_id(), None);
    assert!(owner.begin_delivery(&notification).is_err());
    assert!(owner.completed(ordinary.ticket, notification).is_err());
    assert!(notification_lease.claim(STORAGE_BYTES).is_ok());
    assert!(owner.action()?.is_none());
    let reply = ordinary_reply(1, claim)?;
    let ticket = owner.begin_delivery(&reply)?;
    assert_eq!(ticket, ordinary.ticket);
    owner.completed(ticket, reply)?;
    let next = owner.action()?.ok_or("Missing renewed ordinary credit")?;
    assert_eq!(next.lane, Lane::Ordinary);
    assert_ne!(next.ticket, ordinary.ticket);
    Ok(())
}
