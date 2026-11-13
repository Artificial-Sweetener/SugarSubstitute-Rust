// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Offline files obey the same signed bytes, expiry and cache custody as network delivery.
use super::*;
use crate::setup_storage::{InstallationRun, SelectionStore};
use std::sync::Arc;

#[test]
fn offline_import_resumes_network_prefix_and_keeps_source_pinned_until_verification() -> TestResult
{
    worker::run_reserved(3 * DOWNLOAD_RESERVATION_BYTES, |context, _| {
        let root = installation(101, context)?;
        let external = tempfile::tempdir()?;
        let source = external.path().join("package.bin");
        std::fs::write(&source, PACKAGE)?;
        let mut download = Download::open(storage(root.path(), context)?, target()?, context)?;
        assert!(download.receive(full(), &mut Cursor::new(&PACKAGE[..7]), context).is_err());
        drop(download);
        let store = Arc::new(SelectionStore::open(root.path(), context)?);
        let before = store.load(context)?;
        let _run = InstallationRun::acquire(store.clone(), context)?;
        let mut checks = 0;
        let mut verified = Download::import_file(
            storage(root.path(), context)?,
            target()?,
            &source,
            || {
                checks += 1;
                assert!(std::fs::write(&source, b"changed").is_err());
                assert!(std::fs::remove_file(&source).is_err());
                NOW
            },
            context,
        )?;
        assert_eq!(checks, 3);
        assert_eq!(store.load(context)?, before);
        assert_eq!(std::fs::read(&source)?, PACKAGE);
        std::fs::write(&source, b"source belongs to the user")?;
        let mut bytes = Vec::new();
        verified.read_to_end(&mut bytes)?;
        assert_eq!(bytes, PACKAGE);
        assert!(Download::open(storage(root.path(), context)?, target()?, context).is_err());
        Ok(())
    })
}

#[test]
fn offline_wrong_size_or_directory_preserves_existing_cache_prefix() -> TestResult {
    worker::run_reserved(DOWNLOAD_RESERVATION_BYTES, |context, _| {
        let root = installation(102, context)?;
        let external = tempfile::tempdir()?;
        let source = external.path().join("package.bin");
        let mut download = Download::open(storage(root.path(), context)?, target()?, context)?;
        assert!(download.receive(full(), &mut Cursor::new(&PACKAGE[..5]), context).is_err());
        drop(download);
        for bytes in [b"short".as_slice(), &[PACKAGE, b"extra"].concat()] {
            std::fs::write(&source, bytes)?;
            assert!(matches!(
                Download::import_file(
                    storage(root.path(), context)?,
                    target()?,
                    &source,
                    || NOW,
                    context
                ),
                Err(DownloadFailure::Response(Rejection::BodyLength))
            ));
            assert_eq!(std::fs::read(&source)?, bytes);
            assert_eq!(std::fs::read(payload(root.path())?)?, &PACKAGE[..5]);
        }
        assert!(
            Download::import_file(
                storage(root.path(), context)?,
                target()?,
                external.path(),
                || NOW,
                context
            )
            .is_err()
        );
        assert_eq!(std::fs::read(payload(root.path())?)?, &PACKAGE[..5]);
        Ok(())
    })
}

#[test]
fn offline_corrupt_source_or_cached_prefix_cannot_produce_verified_custody() -> TestResult {
    worker::run_reserved(DOWNLOAD_RESERVATION_BYTES, |context, _| {
        let root = installation(103, context)?;
        let external = tempfile::tempdir()?;
        let source = external.path().join("package.bin");
        let wrong = vec![b'x'; PACKAGE.len()];
        std::fs::write(&source, &wrong)?;
        assert!(matches!(
            Download::import_file(
                storage(root.path(), context)?,
                target()?,
                &source,
                || NOW,
                context
            ),
            Err(DownloadFailure::Verification(Verification::Digest))
        ));
        assert_eq!(std::fs::read(&source)?, wrong);
        assert!(std::fs::read(payload(root.path())?)?.is_empty());
        std::fs::write(&source, PACKAGE)?;
        std::fs::write(payload(root.path())?, b"wrong")?;
        assert!(matches!(
            Download::import_file(
                storage(root.path(), context)?,
                target()?,
                &source,
                || NOW,
                context
            ),
            Err(DownloadFailure::Verification(Verification::Digest))
        ));
        assert!(std::fs::read(payload(root.path())?)?.is_empty());
        let verified = Download::import_file(
            storage(root.path(), context)?,
            target()?,
            &source,
            || NOW,
            context,
        )?;
        assert_eq!(verified.identity().digest(), target()?.digest());
        Ok(())
    })
}

#[test]
fn offline_expiry_before_copy_preserves_prefix_and_expiry_after_copy_preserves_full_cache()
-> TestResult {
    worker::run_reserved(DOWNLOAD_RESERVATION_BYTES, |context, _| {
        let root = installation(104, context)?;
        let external = tempfile::tempdir()?;
        let source = external.path().join("package.bin");
        std::fs::write(&source, PACKAGE)?;
        let mut download = Download::open(storage(root.path(), context)?, target()?, context)?;
        assert!(download.receive(full(), &mut Cursor::new(&PACKAGE[..5]), context).is_err());
        drop(download);
        assert!(matches!(
            Download::import_file(
                storage(root.path(), context)?,
                target()?,
                &source,
                || EXPIRES,
                context
            ),
            Err(DownloadFailure::Verification(Verification::Expired))
        ));
        assert_eq!(std::fs::read(payload(root.path())?)?, &PACKAGE[..5]);
        let mut checks = 0;
        assert!(matches!(
            Download::import_file(
                storage(root.path(), context)?,
                target()?,
                &source,
                || {
                    checks += 1;
                    if checks == 3 { EXPIRES } else { NOW }
                },
                context
            ),
            Err(DownloadFailure::Verification(Verification::Expired))
        ));
        assert_eq!(std::fs::read(payload(root.path())?)?, PACKAGE);
        assert_eq!(std::fs::read(&source)?, PACKAGE);
        Ok(())
    })
}

#[test]
fn offline_cache_alias_is_rejected_without_truncating_either_name() -> TestResult {
    worker::run_reserved(DOWNLOAD_RESERVATION_BYTES, |context, _| {
        let root = installation(105, context)?;
        let external = tempfile::tempdir()?;
        drop(Download::open(storage(root.path(), context)?, target()?, context)?);
        let cached = payload(root.path())?;
        std::fs::write(&cached, PACKAGE)?;
        let alias = external.path().join("alias.bin");
        std::fs::hard_link(&cached, &alias)?;
        for source in [&cached, &alias] {
            assert!(
                Download::import_file(
                    storage(root.path(), context)?,
                    target()?,
                    source,
                    || NOW,
                    context
                )
                .is_err()
            );
            assert_eq!(std::fs::read(source)?, PACKAGE);
        }
        Ok(())
    })
}

#[test]
fn cancelled_offline_import_releases_source_and_leaves_cache_unchanged() -> TestResult {
    worker::run_reserved_expiring(DOWNLOAD_RESERVATION_BYTES, |context, clock| {
        let root = installation(106, context)?;
        let external = tempfile::tempdir()?;
        let source = external.path().join("package.bin");
        std::fs::write(&source, PACKAGE)?;
        let mut download = Download::open(storage(root.path(), context)?, target()?, context)?;
        assert!(download.receive(full(), &mut Cursor::new(&PACKAGE[..5]), context).is_err());
        drop(download);
        assert!(
            Download::import_file(
                storage(root.path(), context)?,
                target()?,
                &source,
                || {
                    clock.expire();
                    NOW
                },
                context
            )
            .is_err()
        );
        assert_eq!(std::fs::read(payload(root.path())?)?, &PACKAGE[..5]);
        std::fs::write(&source, b"unlocked")?;
        Ok(())
    })
}
