// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise exact transfer cursors and immutable deadlines independently of native image decode.
use super::*;
use serde_json::{Value, json};
use sugarsubstitute_contracts::{
    canvas_surface::{CanvasSurfaceMessage, CanvasSurfaceOperation},
    execution::MemoryRelease,
};

struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}
fn memory(bytes: u64) -> MemoryLease {
    MemoryLease::new(bytes, Box::new(Release))
}

fn operation(kind: &str, extra: Value) -> Result<CanvasSurfaceOperation, Box<dyn Error>> {
    let mut value = json!({"protocol":"sugarcubes.canvas","version":3,"session":"assembly-test-session","requestId":1,"revision":4,"type":kind,
        "address":{"sessionId":"canvas:workflow:one","viewId":"view:one","windowId":"window:one","attachmentId":"attachment:one","generation":1}});
    let fields = value.as_object_mut().ok_or("Assembly test envelope is not an object")?;
    for (key, value) in extra.as_object().ok_or("Assembly test data is not an object")? {
        fields.insert(key.clone(), value.clone());
    }
    match CanvasSurfaceMessage::decode(&serde_json::to_vec(&value)?)? {
        CanvasSurfaceMessage::Request(request) => Ok(request.operation().clone()),
        _ => Err("Assembly operation was not decoded".into()),
    }
}
fn begin(length: u64) -> Result<CanvasSurfacePreviewBegin, Box<dyn Error>> {
    match operation(
        "surface.preview.begin",
        json!({"preview":{"generation":0,"promptId":"prompt:one","sourceId":"source:one","runOrder":1,"sequence":1,"token":"preview:1"},"byteLength":length,"format":"jpeg"}),
    )? {
        CanvasSurfaceOperation::PreviewBegin(begin) => Ok(begin),
        _ => Err("Assembly begin was not decoded".into()),
    }
}
fn chunk(
    offset: u64,
    data: String,
    token: &str,
    revision: u64,
) -> Result<CanvasSurfacePreviewChunk, Box<dyn Error>> {
    match operation(
        "surface.preview.chunk",
        json!({"token":token,"leaseRevision":revision,"offset":offset,"data":data}),
    )? {
        CanvasSurfaceOperation::PreviewChunk(chunk) => Ok(chunk),
        _ => Err("Assembly chunk was not decoded".into()),
    }
}

#[test]
fn transfer_accepts_exact_full_chunk_and_tail_without_reallocation_or_deadline_renewal()
-> Result<(), Box<dyn Error>> {
    let memory = memory(1024 * 1024);
    let mut assembly = Assembly::new(begin(8194)?, 3, &memory, Moment::from_nanos(0))?;
    let pointer = assembly.bytes.as_ref().ok_or("Missing assembly allocation")?.0.as_ptr();
    let deadline = assembly.deadline;
    assembly.receive(
        &chunk(0, format!("{}=", "A".repeat(10923)), "preview:1", 3)?,
        Moment::from_nanos(1),
    )?;
    assembly.receive(
        &chunk(8192, "AAA=".into(), "preview:1", 3)?,
        Moment::from_nanos(deadline.as_nanos() - 1),
    )?;
    assert_eq!(assembly.deadline, deadline);
    assert_eq!(assembly.bytes.as_ref().ok_or("Missing assembly allocation")?.0.as_ptr(), pointer);
    assert_eq!(assembly.bytes.as_ref().ok_or("Missing assembly allocation")?.0.len(), 8194);
    assembly.complete("preview:1", 3, Moment::from_nanos(deadline.as_nanos() - 1))?;
    Ok(())
}

#[test]
fn transfer_rejects_foreign_gap_replay_incomplete_and_original_expiry_without_losing_custody()
-> Result<(), Box<dyn Error>> {
    let memory = memory(1024 * 1024);
    let mut assembly = Assembly::new(begin(3)?, 3, &memory, Moment::from_nanos(0))?;
    assert_eq!(
        assembly.complete("preview:1", 3, Moment::from_nanos(0)),
        Err(Rejection::Incomplete)
    );
    assert_eq!(
        assembly.receive(&chunk(0, "AQID".into(), "foreign", 3)?, Moment::from_nanos(0)),
        Err(Rejection::Foreign)
    );
    assert_eq!(
        assembly.receive(&chunk(1, "AQID".into(), "preview:1", 3)?, Moment::from_nanos(0)),
        Err(Rejection::Sequence)
    );
    assembly.receive(&chunk(0, "AQID".into(), "preview:1", 3)?, Moment::from_nanos(0))?;
    assert_eq!(
        assembly.receive(&chunk(0, "AQID".into(), "preview:1", 3)?, Moment::from_nanos(0)),
        Err(Rejection::Sequence)
    );
    assert_eq!(assembly.complete("preview:1", 3, assembly.deadline), Err(Rejection::Expired));
    assert_eq!(assembly.bytes.as_ref().ok_or("Assembly lost rejected bytes")?.0, [1, 2, 3]);
    assert!(assembly.retirement().is_some());
    Ok(())
}

#[test]
fn transfer_rejects_large_protocol_length_before_allocating_outside_actual_admission()
-> Result<(), Box<dyn Error>> {
    let memory = memory(64 * 1024);
    assert!(Assembly::new(begin(64 * 1024 * 1024)?, 3, &memory, Moment::from_nanos(0)).is_err());
    drop(memory.claim(64 * 1024)?);
    Ok(())
}

#[test]
fn declared_jpeg_rejects_png_signature_and_retains_original_allocation_and_claim()
-> Result<(), Box<dyn Error>> {
    let memory = memory(1024 * 1024);
    let mut assembly = Assembly::new(begin(8)?, 3, &memory, Moment::from_nanos(0))?;
    assembly.receive(&chunk(0, "iVBORw0KGgo=".into(), "preview:1", 3)?, Moment::from_nanos(1))?;
    assembly.complete("preview:1", 3, Moment::from_nanos(2))?;
    let pointer = assembly.bytes.as_ref().ok_or("Missing PNG test bytes")?.0.as_ptr();
    let Err(cause) = assembly.original(&memory) else {
        return Err("Declared JPEG accepted PNG bytes".into());
    };
    assert_eq!(
        cause.downcast_ref::<std::io::Error>().map(std::io::Error::kind),
        Some(std::io::ErrorKind::InvalidData)
    );
    let (bytes, claim) = assembly.retirement().as_ref().ok_or("Rejected original lost custody")?;
    assert_eq!(bytes.as_ptr(), pointer);
    assert_eq!(bytes.as_slice(), b"\x89PNG\r\n\x1a\n");
    assert!(claim.bytes() >= bytes.capacity() as u64 + 128);
    assert!(memory.claim(1024 * 1024).is_err());
    drop(assembly);
    drop(memory.claim(1024 * 1024)?);
    Ok(())
}
