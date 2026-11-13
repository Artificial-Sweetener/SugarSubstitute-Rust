// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify native canvas reply and notification correlation and their memory custody.

use super::*;
use serde_json::{Value, json};
use sugarsubstitute_contracts::{
    canvas_native::{
        CanvasBrushOptions, CanvasDimensions, CanvasDocument, CanvasDocumentId, CanvasEditMode,
        CanvasEditState, CanvasFailure, CanvasFailureMessage, CanvasFailureStatus,
        CanvasNativeCapabilities, CanvasRevision, CanvasSourceChange, CanvasSurfaceKey,
        CanvasToolId, CanvasWorkflowSyncedSurface,
    },
    execution::{Generation, MemoryRelease},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    startup::LaunchId,
};

struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}

type TestResult = Result<(), Box<dyn Error>>;

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
fn request(
    identity: MainIdentity,
    kind: &str,
    command: Option<Value>,
) -> Result<CanvasNativeRequest, Box<dyn Error>> {
    let mut value = json!({
        "protocol": "sugarcubes.canvas", "version": 3,
        "session": PanelSession::for_main(identity)?.as_str(),
        "requestId": 2147483647, "type": kind,
    });
    if let Some(command) = command {
        value["command"] = command;
    }
    Ok(CanvasNativeRequest::decode(&serde_json::to_vec(&value)?)?)
}

#[test]
fn terminal_replies_retain_correlation_and_return_the_original_claim() -> TestResult {
    let identity = identity()?;
    let context = request(identity, "context.request", None)?;
    let CanvasNativeRequest::Context(read) = &context else {
        return Err("Expected a context request".into());
    };
    let edit = request(identity, "edit.request", Some(json!({"type": "state"})))?;
    let lease = MemoryLease::new(CanvasNativeReply::STORAGE_BYTES, Box::new(Release));
    let display = CanvasNativeCapabilities { display_output: true, ..Default::default() };
    let unsupported = CanvasFailure::new(CanvasFailureStatus::Unsupported);
    for (payload, kind) in [
        (read.reply(display, None)?, "context.reply"),
        (context.envelope().cancelled_reply()?, "error.reply"),
        (edit.refusal_reply(&unsupported)?, "edit.reply"),
    ] {
        let reply = CanvasNativeReply::new(identity, payload, lease.claim(lease.bytes())?)?;
        assert_eq!(reply.identity(), identity);
        assert_eq!(reply.request_id(), 2147483647);
        let value: Value = serde_json::from_slice(reply.payload())?;
        assert_eq!(value["type"], kind);
        assert_eq!(value["requestId"], 2147483647);
        assert!(lease.claim(1).is_err());
        let claim = reply.into_claim();
        assert_eq!(claim.bytes(), CanvasNativeReply::STORAGE_BYTES);
        drop(claim);
        assert!(lease.claim(lease.bytes()).is_ok());
    }
    Ok(())
}

#[test]
fn replies_reject_foreign_navigation_and_inadequate_custody() -> TestResult {
    let identity = identity()?;
    let request = request(identity, "context.request", None)?;
    let lease = MemoryLease::new(2 * CanvasNativeReply::STORAGE_BYTES, Box::new(Release));
    let cancelled = request.envelope().cancelled_reply()?;
    let short = lease.claim(CanvasNativeReply::STORAGE_BYTES - 1)?;
    assert!(matches!(
        CanvasNativeReply::new(identity, cancelled.clone(), short),
        Err(CanvasNativeReplyError::Storage)
    ));
    let foreign = MainIdentity { navigation: Generation::new(5)?, ..identity };
    let credit = lease.claim(CanvasNativeReply::STORAGE_BYTES)?;
    assert!(matches!(
        CanvasNativeReply::new(foreign, cancelled, credit),
        Err(CanvasNativeReplyError::ForeignNavigation)
    ));
    assert!(lease.claim(lease.bytes()).is_ok());
    Ok(())
}

#[test]
fn large_replies_require_storage_covering_their_payload() -> TestResult {
    let identity = identity()?;
    let surfaces: Vec<Value> = (0..64)
        .map(|index| {
            json!({"key": format!("surface:{index}"), "label": "x".repeat(256),
                "base": {"kind": "blank", "width": 64, "height": 64}, "masks": []})
        })
        .collect();
    let command = json!({"type": "sync", "workflow": "workflow-1", "surfaces": surfaces});
    let CanvasNativeRequest::Workflow(sync) = request(identity, "workflow.request", Some(command))?
    else {
        return Err("Expected a workflow request".into());
    };
    let lease = MemoryLease::new(8 * CanvasNativeReply::STORAGE_BYTES, Box::new(Release));
    let credit = || lease.claim(CanvasNativeReply::STORAGE_BYTES);
    let failure = CanvasFailure {
        status: CanvasFailureStatus::Error,
        message: CanvasFailureMessage::new("Sync failed")?,
    };
    drop(CanvasNativeReply::new(identity, sync.failure_reply(&failure)?, credit()?)?);
    let document = CanvasDocument {
        id: CanvasDocumentId::new("d".repeat(128))?,
        geometry_revision: CanvasRevision::new("g".repeat(512))?,
        size: CanvasDimensions::new(64, 64)?,
    };
    let synced = (0..64)
        .map(|index| {
            Ok(CanvasWorkflowSyncedSurface {
                key: CanvasSurfaceKey::new(format!("surface:{index}"))?,
                document: document.clone(),
                masks: Vec::new(),
            })
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    let large = sync.synced_reply(&synced)?;
    let length = u64::try_from(large.as_bytes().len())?;
    assert!(length > u64::try_from(CanvasNativeReply::INLINE_PAYLOAD_BYTES)?);
    assert!(matches!(
        CanvasNativeReply::new(identity, large.clone(), credit()?),
        Err(CanvasNativeReplyError::Storage)
    ));
    let short = lease.claim(length - 1)?;
    assert!(matches!(
        CanvasNativeReply::with_storage(identity, large.clone(), credit()?, short),
        Err(CanvasNativeReplyError::Storage)
    ));
    let reply = CanvasNativeReply::with_storage(identity, large, credit()?, lease.claim(length)?)?;
    assert_eq!(reply.into_claim().bytes(), CanvasNativeReply::STORAGE_BYTES);
    assert!(lease.claim(lease.bytes()).is_ok());
    Ok(())
}

#[test]
fn notifications_coalesce_by_kind_and_navigation() -> TestResult {
    let identity = identity()?;
    let session = PanelSession::for_main(identity)?;
    let lease = MemoryLease::new(4 * CanvasNativeNotification::STORAGE_BYTES, Box::new(Release));
    let claim = || lease.claim(CanvasNativeNotification::STORAGE_BYTES);
    let change = |revision: &str| -> Result<CanvasNativeNotificationPayload, Box<dyn Error>> {
        let revision = CanvasRevision::new(revision)?;
        Ok(CanvasSourceChange { document: None, revision }.encode(&session)?)
    };
    let first = CanvasNativeNotification::new(identity, change("1")?, claim()?)?;
    let second = CanvasNativeNotification::new(identity, change("2")?, claim()?)?;
    assert!(first.same_target(&second));
    let options = CanvasBrushOptions::new(40, 80, 100)?;
    let state = CanvasEditState {
        revision: 1,
        document: None,
        active_mask: None,
        tool: CanvasToolId::PanZoom,
        tools: Vec::new(),
        brush: options,
        eraser: options,
        selection: None,
        undo: false,
        redo: false,
        session: None,
        mode: CanvasEditMode::None,
    };
    let edit = CanvasNativeNotification::new(identity, state.changed(&session)?, claim()?)?;
    assert_eq!(edit.kind(), CanvasNativeNotificationKind::EditChanged);
    assert!(!first.same_target(&edit));
    let value: Value = serde_json::from_slice(second.payload())?;
    assert_eq!(value["type"], "source.changed");
    assert_eq!(value["requestId"], 0);
    let foreign = MainIdentity { navigation: Generation::new(5)?, ..identity };
    assert!(matches!(
        CanvasNativeNotification::new(foreign, change("3")?, claim()?),
        Err(CanvasNativeReplyError::ForeignNavigation)
    ));
    assert!(matches!(
        CanvasNativeNotification::new(identity, change("3")?, lease.claim(1)?),
        Err(CanvasNativeReplyError::Storage)
    ));
    drop((first, second, edit));
    assert!(lease.claim(lease.bytes()).is_ok());
    Ok(())
}
