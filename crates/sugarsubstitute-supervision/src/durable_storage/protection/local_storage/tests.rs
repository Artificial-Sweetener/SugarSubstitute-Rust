// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual account discovery and protected account namespace custody.
/// A real directory junction cannot enroll an account namespace or alter its authored target.
#[test]
fn account_namespace_rejects_an_actual_junction_before_creating_children()
-> crate::setup_storage::test_worker::TestResult {
    use super::account::namespace_directory::{Namespace, resolve};
    use std::os::windows::fs::MetadataExt;
    crate::setup_storage::test_worker::run_reserved(8 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let account = temporary.path().join("account");
        let target = temporary.path().join("authored");
        std::fs::create_dir(&account)?;
        std::fs::create_dir(&target)?;
        std::fs::write(target.join("preserve"), b"authored")?;
        let link = account.join("SugarSubstitute");
        let output = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", "New-Item -ItemType Junction -Path $env:SUGAR_TEST_JUNCTION -Target $env:SUGAR_TEST_JUNCTION_TARGET -ErrorAction Stop | Out-Null"])
            .env("SUGAR_TEST_JUNCTION", &link)
            .env("SUGAR_TEST_JUNCTION_TARGET", &target)
            .output()?;
        assert!(output.status.success(), "Owned junction fixture creation failed");
        assert_ne!(std::fs::symlink_metadata(&link)?.file_attributes() & 0x400, 0);
        super::super::with(context, |owner| {
            let cause = resolve(
                account,
                Namespace::RuntimeChild("history"),
                true,
                &owner.sid,
                owner.budget.clone(),
                &mut owner.handles,
            )
            .err()
            .ok_or_else(|| std::io::Error::other("Account namespace accepted a junction"))?;
            assert_eq!(cause.operation(), "path_object_type");
            assert_eq!(std::fs::read(target.join("preserve"))?, b"authored");
            assert_eq!(std::fs::read_dir(&target)?.count(), 1);
            Ok(())
        })?;
        Ok(())
    })
}
/// Qualify creation and read-only reopen through the same native account namespace algorithm.
#[test]
fn operational_account_namespace_reopens_exact_identity_and_preserves_missing_history()
-> crate::setup_storage::test_worker::TestResult {
    use super::account::namespace_directory::{Namespace, resolve};
    crate::setup_storage::test_worker::run_reserved(8 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        super::super::with(context, |owner| {
            let initial = std::fs::read_dir(temporary.path())?.count();
            assert!(
                resolve(
                    temporary.path().to_owned(),
                    Namespace::RuntimeChild("history"),
                    false,
                    &owner.sid,
                    owner.budget.clone(),
                    &mut owner.handles
                )
                .is_err()
            );
            assert_eq!(std::fs::read_dir(temporary.path())?.count(), initial);
            let (path, _, identity) = resolve(
                temporary.path().to_owned(),
                Namespace::RuntimeChild("history"),
                true,
                &owner.sid,
                owner.budget.clone(),
                &mut owner.handles,
            )
            .map_err(std::io::Error::other)?;
            let (existing, _, reopened) = resolve(
                temporary.path().to_owned(),
                Namespace::RuntimeChild("history"),
                false,
                &owner.sid,
                owner.budget.clone(),
                &mut owner.handles,
            )
            .map_err(std::io::Error::other)?;
            assert_eq!(existing, path);
            assert_eq!(identity, reopened);
            assert!(std::fs::read_dir(&path)?.next().is_none());
            assert!(!path.join("coordinators").exists());
            assert!(!path.join("backends").exists());
            Ok(())
        })?;
        Ok(())
    })
}
/// Exercise primary-account discovery on the production reservation without changing account data.
#[test]
fn actual_account_folder_resolves_under_the_original_diagnostic_job_reservation()
-> crate::setup_storage::test_worker::TestResult {
    crate::setup_storage::test_worker::run_reserved(8 * 1024 * 1024, |context, _| {
        let path = super::super::with(context, |owner| owner.local_storage(context))?;
        assert!(path.is_absolute());
        assert!(path.is_dir());
        let directory = crate::durable_storage::files::Directory::open(&path, false)?;
        assert_eq!(directory.retained()?.identity()?, directory.identity()?);
        Ok(())
    })
}
