// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Interrupted payload writes never become final generation files.
use super::*;
use crate::setup_storage::files;
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
        let extracted = materialize(&source, temporary.path(), &digest, context)?;
        let Err(cause) = VerifiedBundle::open(&extracted, &digest, context) else {
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
    test_worker::run_reserved(32 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let (manifest, compressed) = test_package::distribution()?;
        let digest = format!("{:x}", Sha256::digest(&manifest));
        let source = temporary.path().join("setup.exe");
        std::fs::write(&source, test_package::seal(&manifest, &compressed))?;
        let extracted = materialize(&source, temporary.path(), &digest, context)?;
        let bundle = VerifiedBundle::open(&extracted, &digest, context)?;
        let destination = temporary.path().join("candidate");
        let interrupted = bundle.payload.copy("app/", &destination, context, |path| {
            let mut file = files::create_new(path)?;
            file.write_all(b"partial write")?;
            file.sync_all()?;
            Err(io::ErrorKind::Interrupted.into())
        });
        assert!(matches!(interrupted, Err(cause) if cause.kind() == io::ErrorKind::Interrupted));
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
    })
}
