// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native read sharing and scratch-file validation preserve entry and authoritative ownership.
use super::*;

#[test]
fn independent_readers_share_native_pins_while_writers_and_cleanup_remain_excluded() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let key = storage_key(b"shared immutable bytes");
        let mut writer = open(root.path(), 16, context)?.acquire(
            Namespace::UpdateOffers,
            &key,
            16,
            100,
            context,
        )?;
        writer.write("payload", b"bytes", context)?;
        drop(writer);
        let mut first = open(root.path(), 16, context)?
            .find(Namespace::UpdateOffers, &key, 100, context)?
            .ok_or("Missing first reader")?;
        let second = open(root.path(), 16, context)?
            .find(Namespace::UpdateOffers, &key, 100, context)?
            .ok_or("Missing second reader")?;
        assert!(first.write("payload", b"changed", context).is_err());
        assert!(first.begin_generated("forbidden", context).is_err());
        assert!(first.record_generated(100, context).is_err());
        assert!(first.declare(["new".to_owned()], context).is_err());
        assert!(
            open(root.path(), 16, context)?
                .acquire(Namespace::UpdateOffers, &key, 16, 100, context)
                .is_err()
        );
        drop(first);
        assert_eq!(second.read("payload", context)?.read_bounded(16)?, b"bytes");
        drop(second);
        assert!(
            open(root.path(), 16, context)?
                .acquire(Namespace::UpdateOffers, &key, 16, 100, context)
                .is_ok()
        );
        Ok(())
    })
}

#[test]
fn interrupted_metadata_slots_recover_without_truncating_hardlinked_authoritative_files()
-> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let mut lease = open(root.path(), 16, context)?.acquire(
            Namespace::UpdateOffers,
            &storage_key(b"metadata recovery"),
            16,
            100,
            context,
        )?;
        let scratch = lease.directory.path().join("record.next");
        protection::with(context, |protection| {
            let mut file = protection.file(&scratch, context)?;
            file.write_all(b"interrupted scratch")?;
            file.sync_all()
        })?;
        lease.write("payload", b"complete", context)?;
        assert!(!scratch.exists());
        let settings = root.path().join("settings");
        protection::with(context, |protection| {
            let mut file = protection.file(&settings, context)?;
            file.write_all(b"authoritative settings")?;
            file.sync_all()
        })?;
        std::fs::hard_link(&settings, &scratch)?;
        assert!(lease.declare(["other".to_owned()], context).is_err());
        assert_eq!(std::fs::read(&settings)?, b"authoritative settings");
        assert_eq!(std::fs::read(&scratch)?, b"authoritative settings");
        Ok(())
    })
}

#[test]
fn cleanup_failure_cannot_be_mistaken_for_benign_native_contention() -> TestResult {
    let primary = crate::ClaimError::new("open_cache", io::Error::from_raw_os_error(32));
    assert!(record::busy(&io::Error::other(primary.clone())));
    let joined = crate::ClaimError::finish::<()>(
        Err(primary),
        Err(crate::ClaimError::new("close_cache", io::Error::from_raw_os_error(5))),
    );
    let cause = io::Error::other(joined.err().ok_or("Missing joined failure")?);
    assert!(!record::busy(&cause));
    assert_eq!(record::kind(&cause), io::ErrorKind::Other);
    assert!(!record::collision(&cause));
    assert_eq!(record::public_error(cause).kind(), io::ErrorKind::Other);
    assert_eq!(
        record::public_error(io::Error::from_raw_os_error(32)).kind(),
        io::ErrorKind::WouldBlock
    );
    Ok(())
}

#[test]
fn foreign_scratch_is_preserved_without_truncation_or_payload_publication() -> TestResult {
    worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let mut lease = open(root.path(), 16, context)?.acquire(
            Namespace::UpdateOffers,
            &storage_key(b"unowned metadata scratch"),
            16,
            100,
            context,
        )?;
        let scratch = lease.directory.path().join("record.next");
        std::fs::write(&scratch, b"unowned scratch")?;
        assert!(lease.write("payload", b"complete", context).is_err());
        assert_eq!(std::fs::read(scratch)?, b"unowned scratch");
        assert!(!lease.path().join("payload").exists());
        Ok(())
    })
}

#[test]
fn unknown_scope_children_are_preserved_and_consume_aggregate_disk_capacity() -> TestResult {
    worker::run_reserved(16 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let cache = open(root.path(), 8, context)?;
        let unknown = cache.path().join("unrecognized-root-file");
        std::fs::write(&unknown, b"preserve")?;
        let report = cache.maintain(100, context)?;
        assert_eq!(report.preserved_entries, 1);
        assert_eq!(report.charged_bytes, 8);
        let rejected = cache.clone().acquire(
            Namespace::UpdateOffers,
            &storage_key(b"new payload"),
            1,
            100,
            context,
        );
        assert_eq!(
            rejected.err().ok_or("Unknown bytes escaped disk admission")?.kind(),
            io::ErrorKind::StorageFull
        );
        assert_eq!(std::fs::read(unknown)?, b"preserve");
        Ok(())
    })
}
