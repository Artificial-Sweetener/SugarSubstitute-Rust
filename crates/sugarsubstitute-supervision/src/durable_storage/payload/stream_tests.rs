// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify bounded streaming through the same native payload used by exact-length updates.
use super::*;
use crate::setup_storage::test_worker::{self as worker, TestResult, pipeline::Pipeline};
const JOB_BYTES: u64 = 4 * 1024 * 1024;

/// Replayed, out-of-order and oversized input cannot change the actual retained prefix.
#[test]
fn streaming_chunks_require_the_physical_end_and_original_byte_extent() -> TestResult {
    worker::run_reserved(JOB_BYTES, |context, _| {
        let root = tempfile::tempdir()?;
        let parent = Directory::open(root.path(), false)?;
        let mut payload = Payload::open(&parent, "payload", 7, context.memory(), context)?;
        payload.append(0, b"abc", context)?;
        for (offset, bytes) in
            [(0, &b"abc"[..]), (4, &b"x"[..]), (3, &b"extra"[..]), (u64::MAX, &b"x"[..])]
        {
            assert!(matches!(payload.append(offset, bytes, context), Err(TransferFailure::Extent)));
            assert_eq!(payload.length()?, 3);
        }
        payload.seek(SeekFrom::Start(0))?;
        payload.append(3, b"defg", context)?;
        payload.flush()?;
        drop(payload);
        assert_eq!(std::fs::read(root.path().join("payload"))?, b"abcdefg");
        Ok(())
    })
}

/// Chunk bounds hold even when the storage grant itself could admit a larger allocation.
#[test]
fn streaming_rejects_excess_chunk_before_file_effects() -> TestResult {
    worker::run_reserved(JOB_BYTES, |context, _| {
        let root = tempfile::tempdir()?;
        let parent = Directory::open(root.path(), false)?;
        let mut payload =
            Payload::open(&parent, "payload", (CHUNK + 2) as u64, context.memory(), context)?;
        let _input = context.memory().claim((CHUNK + 1) as u64).map_err(io::Error::other)?;
        let bytes = vec![1; CHUNK + 1];
        assert!(matches!(payload.append(0, &bytes, context), Err(TransferFailure::Extent)));
        assert_eq!(payload.length()?, 0);
        payload.append(0, &bytes[..CHUNK], context)?;
        payload.flush()?;
        assert_eq!(payload.length()?, CHUNK as u64);
        Ok(())
    })
}

/// Cancellation prevents subsequent writes while still permitting mandatory prefix durability.
#[test]
fn streaming_preserves_the_prefix_when_current_control_expires() -> TestResult {
    worker::run_reserved_expiring(JOB_BYTES, |context, clock| {
        let root = tempfile::tempdir()?;
        let parent = Directory::open(root.path(), false)?;
        let mut payload = Payload::open(&parent, "payload", 7, context.memory(), context)?;
        payload.append(0, b"abc", context)?;
        clock.expire();
        assert!(matches!(payload.append(3, b"defg", context), Err(TransferFailure::Storage(_))));
        payload.flush()?;
        drop(payload);
        assert_eq!(std::fs::read(root.path().join("payload"))?, b"abc");
        Ok(())
    })
}

/// Keep original file and allocation custody while a later job appends without another copy buffer.
#[test]
fn streaming_resumes_across_jobs_with_original_custody() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (root, mut payload) = pipeline.run(JOB_BYTES, |context| {
        let root = tempfile::tempdir()?;
        let parent = Directory::open(root.path(), false)?;
        let mut payload = Payload::open(&parent, "payload", 7, context.memory(), &context)?;
        payload.append(0, b"abc", &context)?;
        Ok((root, payload))
    })?;
    pipeline.run(CUSTODY_BYTES, move |context| {
        assert!(std::fs::write(root.path().join("payload"), b"foreign").is_err());
        payload.append(3, b"defg", &context)?;
        payload.flush()?;
        drop(payload);
        assert_eq!(std::fs::read(root.path().join("payload"))?, b"abcdefg");
        Ok(())
    })
}
