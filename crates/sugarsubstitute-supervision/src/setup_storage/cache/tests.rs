// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real native exclusions, bounded disk admission and restart recovery use execution-owned jobs.
use super::*;
mod generations;
mod ownership;
mod recovery;
mod retirement;
use crate::setup_storage::test_worker::{self as worker, TestResult};
use std::io::{Read, Write};

/// Exercise tiny disk limits through the same production cache admission path.
fn open(path: &Path, bytes: u64, context: &JobContext) -> io::Result<RuntimeCache> {
    RuntimeCache::open(path, CachePolicy { total_bytes: bytes }, context)
}
#[test]
fn retained_native_pin_prevents_expiry_cleanup_and_releases_on_drop() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let key = storage_key(b"feature-compatible-key");
        let mut lease = open(root.path(), 16, context)?.acquire(
            Namespace::UpdateDownloads,
            &key,
            8,
            100,
            context,
        )?;
        lease.write("payload", b"verified", context)?;
        let path = lease.path();
        assert!(
            open(root.path(), 16, context)?
                .find(Namespace::UpdateDownloads, &key, 100, context)
                .is_err()
        );
        let report = open(root.path(), 16, context)?
            .maintain(100 + Namespace::UpdateDownloads.retention(), context)?;
        assert_eq!(report.pinned_entries, 1);
        assert_eq!(report.removed_entries, 0);
        assert_eq!(std::fs::read(path.join("payload"))?, b"verified");
        drop(lease);
        let report = open(root.path(), 16, context)?
            .maintain(100 + Namespace::UpdateDownloads.retention(), context)?;
        assert_eq!(report.removed_entries, 1);
        assert!(!path.exists());
        assert!(
            open(root.path(), 16, context)?
                .find(Namespace::UpdateDownloads, &key, 101, context)?
                .is_none()
        );
        Ok(())
    })
}
#[test]
fn pressure_evicts_oldest_idle_entry_and_never_overcommits_pinned_capacity() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let first = storage_key(b"first");
        let second = storage_key(b"second");
        let third = storage_key(b"third");
        let lease = open(root.path(), 8, context)?.acquire(
            Namespace::UpdateDownloads,
            &first,
            4,
            10,
            context,
        )?;
        let mut idle = open(root.path(), 8, context)?.acquire(
            Namespace::UpdateDownloads,
            &second,
            4,
            20,
            context,
        )?;
        idle.write("payload", b"four", context)?;
        drop(idle);
        let next = open(root.path(), 8, context)?.acquire(
            Namespace::UpdateDownloads,
            &third,
            4,
            30,
            context,
        )?;
        assert!(
            open(root.path(), 8, context)?
                .find(Namespace::UpdateDownloads, &second, 30, context)?
                .is_none()
        );
        assert!(
            open(root.path(), 8, context)?
                .acquire(Namespace::UpdateDownloads, &storage_key(b"fourth"), 1, 30, context)
                .is_err()
        );
        assert!(lease.path().is_dir());
        assert!(next.path().is_dir());
        Ok(())
    })
}
#[test]
fn content_reuse_is_pinned_but_never_grants_feature_authentication() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let key = storage_key(b"owner key and compatibility epoch");
        let mut lease = open(root.path(), 64, context)?.acquire(
            Namespace::UpdateOffers,
            &key,
            32,
            100,
            context,
        )?;
        lease.write("signed", b"untrusted until verified", context)?;
        assert_eq!(lease.verify_storage(context)?, 24);
        drop(lease);
        let lease = open(root.path(), 64, context)?
            .find(Namespace::UpdateOffers, &key, 90, context)?
            .ok_or("Cache absent")?;
        assert_eq!(lease.record.touched, 100);
        let mut bytes = Vec::new();
        lease.read("signed", context)?.read_to_end(&mut bytes)?;
        assert_eq!(bytes, b"untrusted until verified");
        assert!(lease.read("unknown", context).is_err());
        Ok(())
    })
}
#[test]
fn bounded_publication_preserves_previous_complete_bytes_on_rejected_growth() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let mut lease = open(root.path(), 8, context)?.acquire(
            Namespace::UpdateOffers,
            &storage_key(b"route"),
            8,
            100,
            context,
        )?;
        lease.write("signed", b"old", context)?;
        assert!(lease.write("signed", b"too large to store", context).is_err());
        assert_eq!(std::fs::read(lease.path().join("signed"))?, b"old");
        assert!(lease.declare(["../settings".to_owned()], context).is_err());
        assert!(lease.declare(["NUL".to_owned()], context).is_err());
        assert!(lease.declare(["signed/nested".to_owned()], context).is_err());
        Ok(())
    })
}

#[test]
fn invalid_keys_and_budgets_never_create_entries_or_escape_the_cache() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        assert!(open(root.path(), 0, context).is_err());
        for key in ["../settings", "C:/settings", "key", "", &"A".repeat(64)] {
            assert!(
                open(root.path(), 16, context)?
                    .acquire(Namespace::UpdateOffers, key, 1, 100, context)
                    .is_err()
            );
        }
        assert!(
            open(root.path(), 16, context)?
                .acquire(Namespace::UpdateOffers, &storage_key(b"zero"), 0, 100, context)
                .is_err()
        );
        assert!(
            open(root.path(), 16, context)?
                .acquire(Namespace::UpdateOffers, &storage_key(b"large"), 17, 100, context)
                .is_err()
        );
        assert_eq!(
            std::fs::read_dir(
                root.path().join(DIRECTORY).join(Namespace::UpdateOffers.directory())
            )?
            .count(),
            0
        );
        Ok(())
    })
}

#[test]
fn idle_entries_charge_actual_bytes_and_reopening_reserves_capacity_before_growth() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let first = storage_key(b"first");
        let second = storage_key(b"second");
        let mut lease = open(root.path(), 10, context)?.acquire(
            Namespace::UpdateDownloads,
            &first,
            8,
            100,
            context,
        )?;
        lease.write("payload", b"12", context)?;
        drop(lease);
        let second = open(root.path(), 10, context)?.acquire(
            Namespace::UpdateDownloads,
            &second,
            8,
            100,
            context,
        )?;
        assert!(
            open(root.path(), 10, context)?
                .acquire(Namespace::UpdateDownloads, &first, 8, 100, context)
                .is_err()
        );
        drop(second);
        let lease = open(root.path(), 10, context)?
            .find(Namespace::UpdateDownloads, &first, 100, context)?
            .ok_or("Idle cache was lost")?;
        assert_eq!(std::fs::read(lease.path().join("payload"))?, b"12");
        Ok(())
    })
}
