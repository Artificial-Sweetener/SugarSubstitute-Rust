// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Independent replacements and damaged ownership never discard retained compatible bytes.
use super::*;

#[test]
fn fresh_generation_replaces_lookup_without_mutating_or_unpinning_older_readers() -> TestResult {
    worker::run_reserved(48 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let key = storage_key(b"subscription route");
        let mut first = open(root.path(), 4096, context)?.fresh(
            Namespace::UpdateOffers,
            &key,
            16,
            100,
            context,
        )?;
        first.write("offer", b"old bytes", context)?;
        first.publish(context)?;
        drop(first);
        let previous = open(root.path(), 4096, context)?
            .find(Namespace::UpdateOffers, &key, 100, context)?
            .ok_or("Previous absent")?;
        let mut replacement = open(root.path(), 4096, context)?.fresh(
            Namespace::UpdateOffers,
            &key,
            16,
            90,
            context,
        )?;
        replacement.write("offer", b"new bytes", context)?;
        assert_eq!(previous.read("offer", context)?.read_bounded(16)?, b"old bytes");
        replacement.publish(context)?;
        drop(replacement);
        let current = open(root.path(), 4096, context)?
            .find(Namespace::UpdateOffers, &key, 100, context)?
            .ok_or("Current absent")?;
        assert_ne!(previous.path(), current.path());
        assert_eq!(current.read("offer", context)?.read_bounded(16)?, b"new bytes");
        assert_eq!(
            open(root.path(), 4096, context)?
                .maintain(100 + Namespace::UpdateOffers.retention(), context)?
                .pinned_entries,
            2
        );
        drop((previous, current));
        assert_eq!(
            open(root.path(), 4096, context)?
                .maintain(100 + Namespace::UpdateOffers.retention(), context)?
                .removed_entries,
            2
        );
        Ok(())
    })
}
#[test]
fn damaged_record_is_an_observable_miss_and_new_storage_preserves_original_bytes() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let key = storage_key(b"resumable target");
        let mut original = open(root.path(), 4096, context)?.acquire(
            Namespace::UpdateDownloads,
            &key,
            16,
            100,
            context,
        )?;
        original.write("payload", b"preserve", context)?;
        let path = original.path();
        let metadata = original.directory.path().join("record");
        drop(original);
        std::fs::write(&metadata, b"damaged record")?;
        assert!(
            open(root.path(), 4096, context)?
                .find(Namespace::UpdateDownloads, &key, 100, context)?
                .is_none()
        );
        let fresh = open(root.path(), 4096, context)?.acquire(
            Namespace::UpdateDownloads,
            &key,
            16,
            100,
            context,
        )?;
        assert_ne!(fresh.path(), path);
        assert_eq!(std::fs::read(path.join("payload"))?, b"preserve");
        assert_eq!(std::fs::read(metadata)?, b"damaged record");
        Ok(())
    })
}

#[test]
fn relocated_owned_cache_keeps_entries_and_authoritative_siblings_intact() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let first = tempfile::tempdir()?;
        let second = tempfile::tempdir()?;
        std::fs::write(first.path().join("settings"), b"settings")?;
        std::fs::write(second.path().join("user-document"), b"document")?;
        let key = storage_key(b"portable cache identity");
        let mut lease = open(first.path(), 4096, context)?.acquire(
            Namespace::UpdateDownloads,
            &key,
            16,
            100,
            context,
        )?;
        lease.write("payload", b"cache", context)?;
        drop(lease);
        std::fs::rename(first.path().join(DIRECTORY), second.path().join(DIRECTORY))?;
        let lease = open(second.path(), 4096, context)?
            .find(Namespace::UpdateDownloads, &key, 100, context)?
            .ok_or("Relocated entry absent")?;
        assert_eq!(lease.read("payload", context)?.read_bounded(16)?, b"cache");
        assert_eq!(std::fs::read(first.path().join("settings"))?, b"settings");
        assert_eq!(std::fs::read(second.path().join("user-document"))?, b"document");
        Ok(())
    })
}

#[test]
fn cloned_scope_funds_retained_entries_without_retaining_later_job_admission() -> TestResult {
    let mut pipeline = worker::pipeline::Pipeline::new()?;
    let (root, cache) = pipeline.run(16 * 1024 * 1024, |context| {
        let root = tempfile::tempdir()?;
        let cache = open(root.path(), 4096, &context)?;
        Ok((root, cache))
    })?;
    let next = cache.clone();
    let entry = pipeline.run(240 * 1024 * 1024, move |context| {
        let mut entry =
            next.acquire(Namespace::UpdateOffers, &storage_key(b"later job"), 16, 100, &context)?;
        entry.write("offer", b"bytes", &context)?;
        Ok(entry)
    })?;
    // A retained entry must not retain the second job's entire 240 MiB reservation.
    pipeline.run(240 * 1024 * 1024, |_| Ok(()))?;
    drop((entry, cache, root));
    Ok(())
}

#[test]
fn bounded_stream_publication_rejects_short_long_and_failed_writers_before_replacing_target()
-> TestResult {
    worker::run_reserved(16 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let mut entry = open(root.path(), 128, context)?.acquire(
            Namespace::UpdateOffers,
            &storage_key(b"streamed record"),
            128,
            100,
            context,
        )?;
        entry.write("record", b"previous", context)?;
        assert_eq!(
            entry
                .write_with("record", 4, context, |output| output.write_all(b"no"))
                .err()
                .ok_or("Short stream accepted")?
                .kind(),
            io::ErrorKind::UnexpectedEof
        );
        assert_eq!(std::fs::read(entry.path().join("record"))?, b"previous");
        assert_eq!(
            entry
                .write_with("record", 4, context, |output| output.write_all(b"longer"))
                .err()
                .ok_or("Long stream accepted")?
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(std::fs::read(entry.path().join("record"))?, b"previous");
        let failed = entry.write_with("record", 4, context, |output| {
            output.write_all(b"part")?;
            Err(io::Error::from(io::ErrorKind::ConnectionReset))
        });
        assert_eq!(
            failed.err().ok_or("Failed writer accepted")?.kind(),
            io::ErrorKind::ConnectionReset
        );
        assert_eq!(std::fs::read(entry.path().join("record"))?, b"previous");
        entry.write_with("record", 4, context, |output| {
            output.write_all(b"ne")?;
            output.write_all(b"xt")
        })?;
        assert_eq!(std::fs::read(entry.path().join("record"))?, b"next");
        assert!(entry.verify_storage(context)? <= 128);
        Ok(())
    })
}
