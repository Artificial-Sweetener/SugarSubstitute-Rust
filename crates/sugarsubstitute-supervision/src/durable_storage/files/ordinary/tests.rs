// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify actual sharing, selected metadata rights, file-lock lifetime and original controls.
use super::*;
use crate::setup_storage::test_worker::{self as worker, TestResult};
use std::io::Write;

/// Shared metadata rights permit retention updates while denying data writes and exclusion.
#[test]
fn metadata_readers_share_without_granting_data_writes_or_exclusive_access() -> TestResult {
    worker::run_reserved(2 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        std::fs::write(root.path().join("lease"), b"")?;
        let parent = Directory::open(root.path(), false)?;
        let mut reader = parent.open_existing("lease", ExistingAccess::SharedMetadata, context)?;
        let second = parent.open_existing("lease", ExistingAccess::SharedMetadata, context)?;
        assert!(reader.write_all(b"forbidden").is_err());
        assert!(parent.open_existing("lease", ExistingAccess::Exclusive, context).is_err());
        let time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(100);
        set_modified(&reader, time, context)?;
        assert_eq!(modified(&second, context)?, time);
        assert_eq!(reader.metadata()?.len(), 0);
        drop(reader);
        drop(second);
        let exclusive = parent.open_existing("lease", ExistingAccess::Exclusive, context)?;
        assert!(parent.open_existing("lease", ExistingAccess::SharedMetadata, context).is_err());
        drop(exclusive);
        assert_eq!(std::fs::read(root.path().join("lease"))?, b"");
        Ok(())
    })
}

/// Metadata readers deny in-place writers and retire before Windows record publication.
#[test]
fn replaceable_reader_denies_in_place_writes_and_retires_before_publication() -> TestResult {
    worker::run_reserved(2 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let parent = Directory::open(root.path(), false)?;
        crate::durable_storage::protection::with(context, |protection| {
            let mut file = protection.file(&root.path().join("record"), context)?;
            file.write_all(b"original")?;
            file.sync_all()?;
            protection.close_file(file)
        })?;
        let mut reader =
            parent.open_existing("record", ExistingAccess::ReplaceableRead, context)?;
        assert!(parent.open_existing("record", ExistingAccess::Exclusive, context).is_err());
        assert_eq!(read_admitted(&mut reader, 32, context)?, b"original");
        drop(reader);
        crate::durable_storage::records::publish_reusable(
            &parent,
            "record",
            "record.next",
            b"replacement",
            32,
            true,
            context,
        )?;
        assert_eq!(std::fs::read(root.path().join("record"))?, b"replacement");
        Ok(())
    })
}

/// Actual OS exclusion survives its acquisition call and never truncates existing bytes.
#[test]
fn file_exclusion_preserves_existing_bytes_and_releases_only_with_its_guard() -> TestResult {
    worker::run_reserved(2 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        std::fs::write(root.path().join("exclusion"), b"authored")?;
        let parent = Directory::open(root.path(), false)?;
        let first = parent.lock("exclusion", context)?;
        assert!(parent.lock("exclusion", context).is_err());
        drop(first);
        let second = parent.lock("exclusion", context)?;
        drop(second);
        assert_eq!(std::fs::read(root.path().join("exclusion"))?, b"authored");
        Ok(())
    })
}

/// Ended controls prevent creation, opens, retention writes and reads before native effects.
#[test]
fn original_deadline_prevents_every_ordinary_file_effect() -> TestResult {
    worker::run_reserved_expiring(2 * 1024 * 1024, |context, clock| {
        let root = tempfile::tempdir()?;
        std::fs::write(root.path().join("record"), b"preserved")?;
        let parent = Directory::open(root.path(), false)?;
        let mut reader = parent.open_existing("record", ExistingAccess::SharedMetadata, context)?;
        let original = reader.metadata()?.modified()?;
        clock.expire();
        assert_eq!(
            sync(&reader, context).err().ok_or("Expired file flushed")?.kind(),
            io::ErrorKind::TimedOut
        );
        assert_eq!(
            length(&reader, context).err().ok_or("Expired length observed")?.kind(),
            io::ErrorKind::TimedOut
        );
        assert_eq!(
            parent.lock("absent", context).err().ok_or("Expired exclusion acquired")?.kind(),
            io::ErrorKind::TimedOut
        );
        assert_eq!(
            parent
                .open_existing("record", ExistingAccess::Exclusive, context)
                .err()
                .ok_or("Expired file opened")?
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert_eq!(
            read_admitted(&mut reader, 32, context).err().ok_or("Expired record read")?.kind(),
            io::ErrorKind::TimedOut
        );
        assert_eq!(
            set_modified(&reader, std::time::UNIX_EPOCH, context)
                .err()
                .ok_or("Expired metadata changed")?
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert_eq!(reader.metadata()?.modified()?, original);
        assert!(!root.path().join("absent").try_exists()?);
        Ok(())
    })
}
