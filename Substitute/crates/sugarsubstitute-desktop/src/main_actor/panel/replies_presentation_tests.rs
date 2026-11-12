// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise full-snapshot coalescing alongside the original request-response queue.

use super::*;
use crate::SurfacePresentation;
use sugarsubstitute_contracts::canvas_surface::{
    CanvasSurfaceAddress, CanvasSurfacePresentationChange, CanvasSurfacePresentationDisplay,
    CanvasSurfacePresentationStatus,
};

fn snapshot(
    identity: MainIdentity,
    attachment: &str,
    sequence: u64,
    lease: &MemoryLease,
) -> Result<NativeDelivery, Box<dyn Error>> {
    let address = CanvasSurfaceAddress::decode(&serde_json::to_vec(&json!({
        "sessionId":"canvas:w", "viewId":"view:a", "windowId":"window:a",
        "attachmentId":attachment, "generation":1,
    }))?)?;
    let change = CanvasSurfacePresentationChange::new(
        address,
        sequence,
        "preview:one".into(),
        1,
        CanvasSurfacePresentationStatus::Failed,
        CanvasSurfacePresentationDisplay::Empty,
        "".into(),
    )?;
    Ok(NativeDelivery::Presentation(SurfacePresentation::new(
        identity,
        &change,
        lease.claim(SurfacePresentation::STORAGE_BYTES)?,
    )?))
}

#[test]
fn newer_pending_snapshot_releases_replaced_claim_and_preserves_other_message_order()
-> Result<(), Box<dyn Error>> {
    let identity = identity()?;
    let old = MemoryLease::new(SurfacePresentation::STORAGE_BYTES, Box::new(Release));
    let replacement = MemoryLease::new(SurfacePresentation::STORAGE_BYTES, Box::new(Release));
    let others = MemoryLease::new(3 * SurfacePresentation::STORAGE_BYTES, Box::new(Release));
    let mut queue = Replies::new();
    queue.push(context(identity, 1, &others)?)?;
    queue.push(snapshot(identity, "attachment:a", 1, &old)?)?;
    queue.push(snapshot(identity, "attachment:b", 1, &others)?)?;
    queue.push(surface(identity, 2, &others)?)?;
    queue.push(snapshot(identity, "attachment:a", 3, &replacement)?)?;
    assert_eq!(queue.len, 4);
    assert!(old.claim(SurfacePresentation::STORAGE_BYTES).is_ok());
    assert!(replacement.claim(1).is_err());
    let mut order = Vec::new();
    while queue.progress(true, |delivery| {
        let value: Value = serde_json::from_slice(
            delivery.payload().ok_or_else(|| DesktopError::message("test", "Missing payload"))?,
        )
        .map_err(|cause| DesktopError::new("test", cause))?;
        order.push((
            value["type"].clone(),
            value["requestId"].clone(),
            value["change"]["sequence"].clone(),
        ));
        Ok(())
    })? {}
    assert_eq!(
        order,
        vec![
            (json!("context.reply"), json!(1), Value::Null),
            (json!("surface.presentation.changed"), json!(0), json!(3)),
            (json!("surface.presentation.changed"), json!(0), json!(1)),
            (json!("surface.reply"), json!(2), Value::Null),
        ]
    );
    assert!(replacement.claim(SurfacePresentation::STORAGE_BYTES).is_ok());
    assert!(others.claim(3 * SurfacePresentation::STORAGE_BYTES).is_ok());
    Ok(())
}

#[test]
fn pending_snapshot_rejects_older_sequences_and_coalesces_at_full_queue_capacity()
-> Result<(), Box<dyn Error>> {
    let identity = identity()?;
    let lease = MemoryLease::new(
        (CAPACITY as u64 + 1) * SurfacePresentation::STORAGE_BYTES,
        Box::new(Release),
    );
    let mut queue = Replies::new();
    queue.push(snapshot(identity, "attachment:a", 3, &lease)?)?;
    for id in 1..CAPACITY as u64 {
        queue.push(context(identity, id, &lease)?)?;
    }
    for sequence in [3, 2, 4] {
        queue.push(snapshot(identity, "attachment:a", sequence, &lease)?)?;
        assert_eq!(queue.len, CAPACITY);
        assert!(lease.claim(SurfacePresentation::STORAGE_BYTES).is_ok());
    }
    assert!(queue.push(snapshot(identity, "attachment:b", 5, &lease)?).is_err());
    assert!(queue.progress(true, |delivery| {
        let NativeDelivery::Presentation(value) = delivery else {
            return Err(DesktopError::message("test", "Missing snapshot"));
        };
        assert_eq!(value.sequence(), 4);
        assert_eq!(value.identity(), identity);
        Ok(())
    })?);
    queue.clear();
    assert!(lease.claim((CAPACITY as u64 + 1) * SurfacePresentation::STORAGE_BYTES).is_ok());
    Ok(())
}

#[test]
fn distinct_navigations_and_inflight_snapshots_keep_independent_claims_through_cancellation()
-> Result<(), Box<dyn Error>> {
    let identity = identity()?;
    let next = MainIdentity { navigation: Generation::new(2)?, ..identity };
    let lease = MemoryLease::new(3 * SurfacePresentation::STORAGE_BYTES, Box::new(Release));
    let mut queue = Replies::new();
    queue.push(snapshot(identity, "attachment:a", 1, &lease)?)?;
    let mut native = None;
    assert!(queue.progress(true, |delivery| {
        native = Some(delivery);
        Ok(())
    })?);
    queue.push(snapshot(identity, "attachment:a", 2, &lease)?)?;
    queue.push(snapshot(next, "attachment:a", 3, &lease)?)?;
    assert_eq!(queue.len, 2);
    assert!(lease.claim(1).is_err());
    assert!(!queue.progress(false, |_| Ok(()))?);
    queue.clear();
    assert!(lease.claim(2 * SurfacePresentation::STORAGE_BYTES).is_ok());
    assert!(lease.claim(3 * SurfacePresentation::STORAGE_BYTES).is_err());
    drop(native);
    assert!(lease.claim(3 * SurfacePresentation::STORAGE_BYTES).is_ok());
    Ok(())
}

#[test]
fn stale_or_closed_notification_retirement_releases_claims_without_removing_correlated_replies()
-> Result<(), Box<dyn Error>> {
    let identity = identity()?;
    let next = MainIdentity { navigation: Generation::new(2)?, ..identity };
    let lease = MemoryLease::new(3 * SurfacePresentation::STORAGE_BYTES, Box::new(Release));
    let mut queue = Replies::new();
    queue.push(snapshot(identity, "attachment:a", 1, &lease)?)?;
    queue.push(context(identity, 1, &lease)?)?;
    queue.push(snapshot(next, "attachment:a", 2, &lease)?)?;
    assert!(queue.retire_presentations(Some(next))?);
    assert_eq!(queue.len, 2);
    assert!(lease.claim(SurfacePresentation::STORAGE_BYTES).is_ok());
    assert!(!queue.retire_presentations(Some(next))?);
    assert!(queue.retire_presentations(None)?);
    assert_eq!(queue.len, 1);
    assert!(lease.claim(2 * SurfacePresentation::STORAGE_BYTES).is_ok());
    assert!(queue.progress(true, |reply| {
        assert_eq!(reply.request_id(), Some(1));
        Ok(())
    })?);
    assert!(lease.claim(3 * SurfacePresentation::STORAGE_BYTES).is_ok());
    Ok(())
}

fn allowance(
    identity: MainIdentity,
    generation: u64,
    lease: &MemoryLease,
) -> Result<NativeDelivery, Box<dyn Error>> {
    use sugarsubstitute_contracts::memory::{MemoryAllowance, MemoryLevel, MemoryPreferenceChoice};
    let allowance = MemoryAllowance {
        generation,
        level: MemoryLevel::Normal,
        allowance_bytes: 1024,
        ceiling_bytes: 4096,
        total_bytes: 8192,
        preference: MemoryPreferenceChoice::Share { percent: 25 },
    };
    Ok(NativeDelivery::Memory(crate::MemoryCommand::new(
        identity,
        &allowance,
        lease.claim(crate::MemoryCommand::STORAGE_BYTES)?,
    )?))
}

#[test]
fn a_newer_pending_memory_allowance_replaces_the_older_one() -> Result<(), Box<dyn Error>> {
    let identity = identity()?;
    let old = MemoryLease::new(crate::MemoryCommand::STORAGE_BYTES, Box::new(Release));
    let newer = MemoryLease::new(crate::MemoryCommand::STORAGE_BYTES, Box::new(Release));
    let stale = MemoryLease::new(crate::MemoryCommand::STORAGE_BYTES, Box::new(Release));
    let mut queue = Replies::new();
    queue.push(allowance(identity, 1, &old)?)?;
    queue.push(allowance(identity, 2, &newer)?)?;
    // An older generation never replaces a newer pending allowance.
    queue.push(allowance(identity, 1, &stale)?)?;
    assert_eq!(queue.len, 1);
    assert!(old.claim(crate::MemoryCommand::STORAGE_BYTES).is_ok());
    assert!(stale.claim(crate::MemoryCommand::STORAGE_BYTES).is_ok());
    let mut generations = Vec::new();
    while queue.progress(true, |delivery| {
        let value: Value = serde_json::from_slice(
            delivery.payload().ok_or_else(|| DesktopError::message("test", "Missing payload"))?,
        )
        .map_err(|cause| DesktopError::new("test", cause))?;
        generations.push(value["generation"].as_u64());
        assert!(delivery.lane().is_none());
        Ok(())
    })? {}
    assert_eq!(generations, [Some(2)]);
    Ok(())
}
