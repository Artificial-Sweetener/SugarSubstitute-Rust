// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify exact context cancellation correlation and native completion custody.

use super::*;
use serde_json::{Value, json};
use sugarsubstitute_contracts::{
    execution::{Generation, MemoryRelease},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    startup::LaunchId,
};

struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}

/// Construct the exact navigation encoded in the native panel session.
fn identity() -> Result<MainIdentity, Box<dyn Error>> {
    Ok(MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([7; 32]),
                epoch: CoordinatorEpoch::new([8; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(2)?,
        },
        launch: LaunchId::new(3)?,
        navigation: Generation::new(4)?,
    })
}

/// Qualify a public request through the same shared decoder used by native ingress.
fn request(identity: MainIdentity) -> Result<CanvasContextRequest, Box<dyn Error>> {
    Ok(CanvasContextRequest::decode(&serde_json::to_vec(&json!({
        "protocol": "sugarcubes.canvas", "version": 3,
        "session": PanelSession::for_main(identity)?.as_str(),
        "requestId": 2147483647, "type": "context.request",
    }))?)?)
}

#[test]
fn context_terminal_replies_retain_correlation_and_original_claim() -> Result<(), Box<dyn Error>> {
    let identity = identity()?;
    let request = request(identity)?;
    let lease = MemoryLease::new(CanvasContextReply::STORAGE_BYTES, Box::new(Release));
    for cancelled in [false, true] {
        let claim = lease.claim(lease.bytes())?;
        let reply = if cancelled {
            CanvasContextReply::cancelled(identity, &request, claim)?
        } else {
            CanvasContextReply::new(identity, &request, CanvasDisplayCapability::Available, claim)?
        };
        assert_eq!(reply.identity(), identity);
        assert_eq!(reply.request_id(), request.request_id());
        let value: Value = serde_json::from_slice(reply.payload())?;
        assert_eq!(value["session"], request.session().as_str());
        assert_eq!(value["requestId"], request.request_id());
        if cancelled {
            assert_eq!(value["type"], "error.reply");
            assert_eq!(value["failure"], json!({ "status": "cancelled", "message": "" }));
            assert!(value.get("context").is_none());
        } else {
            assert_eq!(value["type"], "context.reply");
        }
        assert!(lease.claim(1).is_err());
        let claim = reply.into_claim();
        assert_eq!(claim.bytes(), CanvasContextReply::STORAGE_BYTES);
        assert!(lease.claim(1).is_err());
        drop(claim);
        assert!(lease.claim(lease.bytes()).is_ok());
    }
    Ok(())
}

#[test]
fn cancellation_rejects_foreign_navigation_and_inadequate_custody() -> Result<(), Box<dyn Error>> {
    let identity = identity()?;
    let request = request(identity)?;
    let lease = MemoryLease::new(CanvasContextReply::STORAGE_BYTES, Box::new(Release));
    assert!(matches!(
        CanvasContextReply::cancelled(identity, &request, lease.claim(lease.bytes() - 1)?),
        Err(CanvasContextReplyError::Storage)
    ));
    let foreign = MainIdentity { navigation: Generation::new(5)?, ..identity };
    assert!(matches!(
        CanvasContextReply::cancelled(foreign, &request, lease.claim(lease.bytes())?),
        Err(CanvasContextReplyError::ForeignNavigation)
    ));
    assert!(lease.claim(lease.bytes()).is_ok());
    Ok(())
}
