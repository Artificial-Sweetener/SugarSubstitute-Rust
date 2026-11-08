// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit exact ordered UTF-8 bytes and retain memory through decoded capture ownership.
use super::*;
use sha2::{Digest, Sha256};
use sugarsubstitute_application::workspace_session::CaptureTicket;
use sugarsubstitute_contracts::desktop_session::{
    CheckpointId, encode_checkpoint,
    wire::{SessionFence, SessionTransfer},
};
struct Release;
impl MemoryRelease for Release {
    fn release(&self, _: u64) {}
}
fn input() -> TestResult<(CaptureTicket, SessionCheckpoint, SessionTransfer, Vec<u8>, MemoryLease)>
{
    let checkpoint = checkpoint(10)?;
    let ticket = CaptureTicket {
        checkpoint: CheckpointId::new(checkpoint.id.clone())?,
        revision: 10,
        fence: SessionFence {
            webview: "00000000-0000-4000-8000-000000000001".into(),
            revision: 2,
            bindings: vec![],
        },
        closing: false,
    };
    let bytes = encode_checkpoint(&checkpoint)?;
    let metadata = SessionTransfer {
        id: "00000000-0000-4000-8000-000000000002".into(),
        checkpoint_id: checkpoint.id.clone(),
        revision: 10,
        bytes: bytes.len() as u64,
        chunks: 1,
        digest: files::hex(&Sha256::digest(&bytes)),
        direction: "capture".into(),
    };
    Ok((ticket, checkpoint, metadata, bytes, MemoryLease::new(8 * 1024 * 1024, Box::new(Release))))
}
#[test]
fn transfer_commits_exact_snapshot_and_refuses_repeated_or_incomplete_chunks() -> TestResult {
    let (ticket, checkpoint, metadata, bytes, memory) = input()?;
    let mut transfer = CaptureTransfer::new(
        metadata.clone(),
        &ticket,
        checkpoint.scope.clone(),
        Moment::from_nanos(100),
        Moment::from_nanos(0),
        &memory,
    )?;
    assert_eq!(
        transfer.chunk(&metadata.id, 1, "wrong", Moment::from_nanos(1)),
        Err(TransferError::Order)
    );
    transfer.chunk(&metadata.id, 0, std::str::from_utf8(&bytes)?, Moment::from_nanos(1))?;
    assert_eq!(
        transfer.chunk(&metadata.id, 0, "duplicate", Moment::from_nanos(1)),
        Err(TransferError::Order)
    );
    assert_eq!(transfer.finish(&metadata.id, Moment::from_nanos(2))?.checkpoint, checkpoint);
    let transfer = CaptureTransfer::new(
        metadata.clone(),
        &ticket,
        checkpoint.scope.clone(),
        Moment::from_nanos(100),
        Moment::from_nanos(0),
        &memory,
    )?;
    assert!(matches!(
        transfer.finish(&metadata.id, Moment::from_nanos(2)),
        Err(TransferError::Order)
    ));
    Ok(())
}
#[test]
fn digest_revision_scope_memory_and_original_deadline_are_independently_admitted() -> TestResult {
    let (ticket, checkpoint, mut metadata, bytes, memory) = input()?;
    metadata.revision += 1;
    assert!(matches!(
        CaptureTransfer::new(
            metadata.clone(),
            &ticket,
            checkpoint.scope.clone(),
            Moment::from_nanos(100),
            Moment::from_nanos(0),
            &memory
        ),
        Err(TransferError::Invalid)
    ));
    metadata.revision -= 1;
    metadata.digest = "f".repeat(64);
    let mut transfer = CaptureTransfer::new(
        metadata.clone(),
        &ticket,
        checkpoint.scope.clone(),
        Moment::from_nanos(100),
        Moment::from_nanos(0),
        &memory,
    )?;
    transfer.chunk(&metadata.id, 0, std::str::from_utf8(&bytes)?, Moment::from_nanos(1))?;
    assert!(matches!(
        transfer.finish(&metadata.id, Moment::from_nanos(2)),
        Err(TransferError::Integrity)
    ));
    let mut transfer = CaptureTransfer::new(
        metadata.clone(),
        &ticket,
        checkpoint.scope.clone(),
        Moment::from_nanos(100),
        Moment::from_nanos(0),
        &memory,
    )?;
    assert_eq!(
        transfer.chunk(&metadata.id, 0, "late", Moment::from_nanos(100)),
        Err(TransferError::Deadline)
    );
    let tiny = MemoryLease::new(1, Box::new(Release));
    assert!(matches!(
        CaptureTransfer::new(
            metadata,
            &ticket,
            checkpoint.scope,
            Moment::from_nanos(100),
            Moment::from_nanos(0),
            &tiny
        ),
        Err(TransferError::Memory)
    ));
    Ok(())
}
