// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Protected originals cannot reuse cached pixels or replay descriptors past their admission.
use super::*;
use crate::startup::canvas::session_source::{OriginalSpec, SessionSource};
use sha2::{Digest, Sha256};
use viewport::fixture::Fixture;

const SOURCE_METADATA_BYTES: u64 = 4 * 1024 * 1024;

/// Model WorkspaceWork's independent 24 MiB delivery lease, separate from native IPC credit.
fn source_memory(fixture: &Fixture) -> MemoryLease {
    let memory = MemoryLease::new(24 * 1024 * 1024, Box::new(Release));
    assert!(!memory.same_reservation(&fixture.memory));
    assert_eq!(fixture.memory.bytes(), 128 * 1024);
    assert!(fixture.memory.claim(SOURCE_METADATA_BYTES).is_err());
    memory
}

/// A refused original consumes only its separately admitted source metadata reservation.
fn unavailable(memory: &MemoryLease) -> Result<SessionSource, Box<dyn Error>> {
    Ok(SessionSource::new(vec![OriginalSpec::Unavailable], memory.claim(SOURCE_METADATA_BYTES)?))
}

/// A receipt for the real encoded fixture original, independent of its authored document.
fn retained(fixture: &Fixture, memory: &MemoryLease) -> Result<SessionSource, Box<dyn Error>> {
    let path = fixture.output("wide.png");
    let bytes = std::fs::read(&path)?;
    let digest: String = Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect();
    let asset = sugarsubstitute_contracts::desktop_session::wire::SessionAsset {
        version: 1,
        digest,
        bytes: bytes.len() as u64,
        extension: "png".into(),
        media: "image".into(),
    };
    Ok(SessionSource::new(
        vec![OriginalSpec::Retained { path, asset }],
        memory.claim(SOURCE_METADATA_BYTES)?,
    ))
}

/// Establish an actual selected document and host-owned selection correlation.
fn protected_fixture() -> Result<(Fixture, MemoryLease), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    fixture.settle()?;
    let memory = source_memory(&fixture);
    let ingress = fixture.memory.claimed_bytes();
    fixture.canvas.set_session_source(Some(retained(&fixture, &memory)?));
    assert_eq!(fixture.memory.claimed_bytes(), ingress);
    fixture.select(5, "wide.png")?;
    fixture.canvas.set_session_source(None);
    fixture.complete(5)?;
    fixture.settle()?;
    assert_eq!(memory.claimed_bytes(), SOURCE_METADATA_BYTES);
    Ok((fixture, memory))
}

/// Submit the public Output route command without implicitly resolving a host replay.
fn activate(fixture: &mut Fixture, id: u64) -> Result<(), Box<dyn Error>> {
    let request = CanvasNativeRequest::decode(&serde_json::to_vec(&json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":fixture.panel.as_str(),
        "requestId":id, "type":"workflow.request", "command":{
            "type":"activate", "workflow":"workflow-test", "route":"output", "surface":null, "mask":null
        }
    }))?)?;
    fixture.canvas.native(
        fixture.identity,
        request,
        fixture.memory.claim(64 * 1024)?,
        Default::default(),
    )
}

/// Observe the original workflow request's typed terminal refusal.
fn refused(fixture: &mut Fixture, id: u64) -> Result<(), Box<dyn Error>> {
    let Some(DesktopStartupCommand::SendCanvasNative(reply)) = fixture.canvas.take_command() else {
        return Err("Missing protected output route refusal".into());
    };
    let value: Value = serde_json::from_slice(reply.payload())?;
    assert_eq!(reply.identity(), fixture.identity);
    assert_eq!(value["protocol"], "sugarcubes.canvas");
    assert_eq!(value["version"], 3);
    assert_eq!(value["session"], fixture.panel.as_str());
    assert_eq!(value["requestId"], id);
    assert_eq!(value["type"], "workflow.reply");
    assert_eq!(value["result"]["kind"], "failure");
    assert_eq!(value["result"]["status"], "unavailable");
    Ok(())
}

#[test]
fn unavailable_original_refuses_before_reselecting_retained_pixels() -> Result<(), Box<dyn Error>> {
    let mut fixture = Fixture::new()?;
    fixture.commit(4)?;
    fixture.settle()?;
    let previous = fixture.accepted()?;
    let memory = source_memory(&fixture);
    let ingress = fixture.memory.claimed_bytes();
    fixture.canvas.set_session_source(Some(unavailable(&memory)?));
    assert_eq!(memory.claimed_bytes(), SOURCE_METADATA_BYTES);
    assert_eq!(fixture.memory.claimed_bytes(), ingress);
    fixture.select(5, "wide.png")?;
    fixture.take(5, "unavailable")?;
    fixture.canvas.set_session_source(None);
    assert_eq!(fixture.accepted()?, previous);
    fixture.finish_resident(memory)
}

#[test]
fn protected_parked_output_waits_for_current_uuid_revalidation() -> Result<(), Box<dyn Error>> {
    let (mut fixture, memory) = protected_fixture()?;
    let previous = fixture.accepted()?;
    activate(&mut fixture, 100)?;
    assert!(
        fixture.canvas.take_command().is_none(),
        "Parked output bypassed the current original decision"
    );
    let replay = fixture.canvas.output_replay().ok_or("Missing owned output replay")?;
    assert!(
        matches!(replay.selection.operation(), sugarsubstitute_contracts::canvas_surface::CanvasSurfaceOperation::Select { item_id, .. } if item_id == "result:wide.png")
    );
    fixture.canvas.resume_output_replay(replay, None)?;
    refused(&mut fixture, 100)?;
    assert_eq!(fixture.accepted()?, previous);
    fixture.finish_resident(memory)
}

#[test]
fn superseded_output_replay_never_retargets_a_new_selection() -> Result<(), Box<dyn Error>> {
    let (mut fixture, memory) = protected_fixture()?;
    activate(&mut fixture, 100)?;
    let replay = fixture.canvas.output_replay().ok_or("Missing owned output replay")?;
    fixture.select(6, "tall.png")?;
    fixture.complete(6)?;
    fixture.settle()?;
    let previous = fixture.accepted()?;
    let source = retained(&fixture, &memory)?;
    fixture.canvas.resume_output_replay(replay, Some(source))?;
    refused(&mut fixture, 100)?;
    assert_eq!(fixture.accepted()?, previous);
    fixture.finish_resident(memory)
}

#[test]
fn cancelled_output_replay_releases_its_exact_held_request() -> Result<(), Box<dyn Error>> {
    let (mut fixture, memory) = protected_fixture()?;
    activate(&mut fixture, 100)?;
    assert!(fixture.canvas.output_replay().is_some());
    let previous = fixture.accepted()?;
    let ingress = fixture.memory.claimed_bytes();
    let cancellation = MemoryLease::new(
        sugarsubstitute_desktop::PanelIngressReceipt::STORAGE_BYTES,
        Box::new(Release),
    );
    assert!(!cancellation.same_reservation(&fixture.memory));
    let message = CanvasSurfaceMessage::decode(&serde_json::to_vec(&json!({
        "protocol":"sugarcubes.canvas", "version":3, "session":fixture.panel.as_str(),
        "requestId":100, "type":"cancel"
    }))?)?;
    fixture.canvas.surface(
        fixture.identity,
        message,
        None,
        cancellation.claim(cancellation.bytes())?,
    )?;
    assert_eq!(fixture.memory.claimed_bytes(), ingress);
    assert_eq!(cancellation.claimed_bytes(), cancellation.bytes());
    assert!(fixture.canvas.output_replay().is_none());
    let Some(DesktopStartupCommand::SendCanvasNative(reply)) = fixture.canvas.take_command() else {
        return Err("Missing output route cancellation".into());
    };
    let value: Value = serde_json::from_slice(reply.payload())?;
    assert_eq!(reply.identity(), fixture.identity);
    assert_eq!(value["protocol"], "sugarcubes.canvas");
    assert_eq!(value["version"], 3);
    assert_eq!(value["session"], fixture.panel.as_str());
    assert_eq!(value["requestId"], 100);
    assert_eq!(value["type"], "error.reply");
    assert_eq!(value["failure"]["status"], "cancelled");
    drop(reply);
    drop(fixture.memory.claim(fixture.memory.bytes())?);
    assert_eq!(cancellation.claimed_bytes(), cancellation.bytes());
    cancel_consumed(&mut fixture.canvas, fixture.identity)?;
    assert_eq!(cancellation.claimed_bytes(), 0);
    assert_eq!(fixture.accepted()?, previous);
    fixture.finish_resident(memory)
}
