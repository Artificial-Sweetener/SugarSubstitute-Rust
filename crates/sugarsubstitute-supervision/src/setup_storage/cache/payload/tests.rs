// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify actual cross-job cache funding and declared resumable-file access.
use super::*;
use crate::setup_storage::test_worker::{TestResult, pipeline::Pipeline};

/// Fill actual execution admission after a transient opening job while retaining native custody.
#[test]
fn payload_custody_retains_the_cache_reservation_without_retaining_the_opening_job() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (root, lease) = pipeline.run(32 * 1024 * 1024, |context| {
        let root = tempfile::tempdir()?;
        let cache = RuntimeCache::open(root.path(), CachePolicy { total_bytes: 32 }, &context)?;
        let mut lease = cache.acquire(
            Namespace::UpdateDownloads,
            &storage_key(b"retained payload"),
            7,
            100,
            &context,
        )?;
        lease.declare(["payload".into()], &context)?;
        Ok((root, lease))
    })?;
    let (root, lease, payload) = pipeline.run(8 * 1024 * 1024, move |context| {
        let mut payload = lease.payload("payload", 7, &context)?;
        payload.receive_exact(0, 7, &mut std::io::Cursor::new(b"initial"), &context)?;
        Ok((root, lease, payload))
    })?;
    // The original 32 MiB cache and this actual 224 MiB job fill the unchanged executor limit.
    // Retaining the intervening 8 MiB opening job would prevent this admission from completing.
    pipeline.run(224 * 1024 * 1024, move |_| {
        assert_eq!(payload.length()?, 7);
        assert!(std::fs::write(lease.path().join("payload"), b"changed").is_err());
        let path = lease.path().join("payload");
        drop((payload, lease));
        assert_eq!(std::fs::read(path)?, b"initial");
        drop(root);
        Ok(())
    })
}

/// Preserve cache contents when declaration, extent or immutable-reader authority rejects access.
#[test]
fn undeclared_or_overgrant_payloads_and_immutable_readers_reject_before_file_creation() -> TestResult
{
    let mut pipeline = Pipeline::new()?;
    pipeline.run(32 * 1024 * 1024, |context| {
        let root = tempfile::tempdir()?;
        let cache = RuntimeCache::open(root.path(), CachePolicy { total_bytes: 32 }, &context)?;
        let mut lease = cache.acquire(
            Namespace::UpdateDownloads,
            &storage_key(b"payload declaration"),
            7,
            100,
            &context,
        )?;
        assert!(lease.payload("payload", 7, &context).is_err());
        lease.declare(["payload".into()], &context)?;
        assert!(lease.payload("payload", 8, &context).is_err());
        assert!(lease.payload("../escape", 7, &context).is_err());
        assert!(std::fs::read_dir(lease.path())?.next().transpose()?.is_none());
        lease.write("payload", b"initial", &context)?;
        lease.publish(&context)?;
        let reader = lease.into_reader(&context)?;
        assert!(reader.payload("payload", 7, &context).is_err());
        assert_eq!(std::fs::read(reader.path().join("payload"))?, b"initial");
        Ok(())
    })
}
