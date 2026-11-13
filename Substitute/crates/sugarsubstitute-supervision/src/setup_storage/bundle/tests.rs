// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Interrupted payload writes never become final generation files.
use super::*;
use crate::durable_storage::files;
use crate::setup_storage::{materialize, test_package, test_worker};
use sha2::{Digest, Sha256};
use std::io::Write;
mod launcher;

#[test]
fn malformed_authenticated_application_contract_never_becomes_a_verified_bundle()
-> test_worker::TestResult {
    test_worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let (manifest, compressed) = test_package::distribution_with_contract(Some(
            br#"{"schema_version":2,"installed_launch_protocols":[1]}"#,
        ))?;
        let digest = format!("{:x}", Sha256::digest(&manifest));
        let source = temporary.path().join("setup.exe");
        std::fs::write(&source, test_package::seal(&manifest, &compressed))?;
        let Err(cause) = materialize(
            &source,
            &crate::setup_storage::cache::RuntimeCache::open(
                temporary.path(),
                crate::setup_storage::cache::CachePolicy::default(),
                context,
            )?,
            &digest,
            context,
        ) else {
            return Err("Malformed contract admitted".into());
        };
        assert_eq!(
            cause
                .get_ref()
                .and_then(|cause| cause
                    .downcast_ref::<sugarsubstitute_contracts::application_launch::Rejection>()),
            Some(&sugarsubstitute_contracts::application_launch::Rejection::Format),
        );
        Ok(())
    })
}

#[test]
fn interrupted_copy_retains_only_temporary_bytes_and_retry_completes_exact_payloads()
-> test_worker::TestResult {
    test_worker::run_reserved(
        crate::setup_storage::MATERIALIZATION_RESERVATION_BYTES
            + crate::setup_storage::RESERVATION_BYTES,
        |context, _| {
            let temporary = tempfile::tempdir()?;
            let (manifest, compressed) = test_package::distribution()?;
            let digest = format!("{:x}", Sha256::digest(&manifest));
            let source = temporary.path().join("setup.exe");
            std::fs::write(&source, test_package::seal(&manifest, &compressed))?;
            let extracted = materialize(
                &source,
                &crate::setup_storage::cache::RuntimeCache::open(
                    temporary.path(),
                    crate::setup_storage::cache::CachePolicy::default(),
                    context,
                )?,
                &digest,
                context,
            )?;
            let bundle = extracted;
            let destination = temporary.path().join("candidate");
            let interrupted = bundle.payload.copy("app/", &destination, context, |path| {
                let mut file = files::create_new(path)?;
                file.write_all(b"partial write")?;
                file.sync_all()?;
                Err(io::ErrorKind::Interrupted.into())
            });
            assert!(
                matches!(interrupted, Err(cause) if cause.kind() == io::ErrorKind::Interrupted)
            );
            for entry in bundle.payload.inventory.entries() {
                if let Some(relative) = entry.path().strip_prefix("app/") {
                    assert!(!destination.join(relative).exists());
                }
            }
            let orphan =
                std::fs::read_dir(&destination)?.next().ok_or("Interrupted file missing")??.path();
            bundle.payload.copy("app/", &destination, context, files::create_new)?;
            let _verified =
                VerifiedRuntime::verify_copy(&bundle.payload.inventory, &destination, context)?;
            assert_eq!(std::fs::read(orphan)?, b"partial write");
            Ok(())
        },
    )
}

/// A bundle extracted from the test package, for copy tests.
fn extracted(
    temporary: &Path,
    context: &JobContext,
) -> Result<super::VerifiedBundle, Box<dyn std::error::Error + Send + Sync>> {
    let (manifest, compressed) = test_package::distribution()?;
    let digest = format!("{:x}", Sha256::digest(&manifest));
    let source = temporary.join("setup.exe");
    std::fs::write(&source, test_package::seal(&manifest, &compressed))?;
    let cache = crate::setup_storage::cache::RuntimeCache::open(
        temporary,
        crate::setup_storage::cache::CachePolicy::default(),
        context,
    )?;
    Ok(materialize(&source, &cache, &digest, context)?)
}

#[test]
fn a_copy_that_fails_after_creating_its_stage_file_leaves_no_stage_file_behind()
-> test_worker::TestResult {
    test_worker::run_reserved(
        crate::setup_storage::MATERIALIZATION_RESERVATION_BYTES
            + crate::setup_storage::RESERVATION_BYTES,
        |context, _| {
            let temporary = tempfile::tempdir()?;
            let bundle = extracted(temporary.path(), context)?;
            let destination = temporary.path().join("candidate");
            // The stage file exists, but its handle cannot write, so the copy fails mid-file.
            let failed = bundle.payload.copy("app/", &destination, context, |path| {
                drop(files::create_new(path)?);
                std::fs::OpenOptions::new().read(true).open(path)
            });
            assert!(failed.is_err(), "a read-only stage handle cannot be written");
            assert_eq!(std::fs::read_dir(&destination)?.count(), 0, "no stage file remains");
            bundle.payload.copy("app/", &destination, context, files::create_new)?;
            VerifiedRuntime::verify_copy(&bundle.payload.inventory, &destination, context)?;
            Ok(())
        },
    )
}

#[test]
fn a_short_disk_refuses_the_copy_before_writing_with_a_typed_shortage() -> test_worker::TestResult {
    struct Full;
    impl crate::setup_storage::FreeSpace for Full {
        fn available(&self, _: &Path) -> io::Result<u64> {
            Ok(0)
        }
    }
    test_worker::run_reserved(
        crate::setup_storage::MATERIALIZATION_RESERVATION_BYTES
            + crate::setup_storage::RESERVATION_BYTES,
        |context, _| {
            let temporary = tempfile::tempdir()?;
            let bundle = extracted(temporary.path(), context)?;
            let destination = temporary.path().join("candidate");
            let Err(refused) =
                bundle.payload.copy_with("app/", &destination, context, files::create_new, &Full)
            else {
                return Err("a full disk admitted the copy".into());
            };
            let bytes: u64 = bundle
                .payload
                .inventory
                .entries()
                .iter()
                .filter(|entry| entry.path().starts_with("app/"))
                .map(|entry| entry.bytes())
                .sum();
            assert_eq!(
                crate::setup_storage::SpaceShortage::find(&refused),
                Some(crate::setup_storage::SpaceShortage {
                    needed: bytes + crate::setup_storage::HEADROOM_BYTES,
                    available: 0
                })
            );
            assert_eq!(std::fs::read_dir(&destination)?.count(), 0, "nothing was written");
            Ok(())
        },
    )
}
