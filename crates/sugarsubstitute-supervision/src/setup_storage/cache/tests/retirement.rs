// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Feature retirement journals intent while native pins and exact inventories control deletion.
use super::*;

#[test]
fn requested_retirement_preserves_pins_and_other_namespaces_then_reclaims_young_output()
-> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let cache = open(root.path(), 64, context)?;
        let mut lease = cache.clone().acquire(
            Namespace::SetupDownloads,
            &storage_key(b"retire"),
            16,
            100,
            context,
        )?;
        lease.write("payload", b"reclaim", context)?;
        let path = lease.path();
        let mut other = cache.clone().acquire(
            Namespace::UpdateOffers,
            &storage_key(b"other"),
            8,
            100,
            context,
        )?;
        other.write("payload", b"keep", context)?;
        let preserved = other.path();
        drop(other);
        lease.retire(context)?;
        lease.retire(context)?;
        assert_eq!(cache.maintain(100, context)?.removed_entries, 0);
        assert_eq!(std::fs::read(path.join("payload"))?, b"reclaim");
        drop(lease);
        assert_eq!(cache.maintain(100, context)?.removed_entries, 1);
        assert!(!path.exists());
        assert_eq!(std::fs::read(preserved.join("payload"))?, b"keep");
        Ok(())
    })
}

#[test]
fn retirement_rejects_unsettled_producers_and_unknown_files() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let cache = open(root.path(), 64, context)?;
        let mut lease = cache.clone().acquire(
            Namespace::SetupDownloads,
            &storage_key(b"unknown"),
            16,
            100,
            context,
        )?;
        lease.begin_generated("still-running", context)?;
        assert_eq!(
            lease.retire(context).err().ok_or("Unsettled producer retired")?.kind(),
            io::ErrorKind::WouldBlock
        );
        std::fs::write(lease.path().join("owned"), b"owned")?;
        lease.record_generated(100, context)?;
        std::fs::write(lease.path().join("foreign"), b"preserve")?;
        assert!(lease.retire(context).is_err());
        let path = lease.path();
        drop(lease);
        assert_eq!(cache.maintain(100, context)?.removed_entries, 0);
        assert_eq!(std::fs::read(path.join("foreign"))?, b"preserve");
        Ok(())
    })
}

#[test]
fn registered_namespace_is_created_when_opening_an_older_owned_scope() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let cache = open(root.path(), 64, context)?;
        let directory = cache.path().join(Namespace::SetupRuntime.directory());
        drop(cache);
        std::fs::remove_dir(&directory)?;
        let cache = open(root.path(), 64, context)?;
        assert!(directory.is_dir());
        assert_eq!(cache.maintain(100, context)?.preserved_entries, 0);
        Ok(())
    })
}
