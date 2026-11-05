// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise installation-local account runtime storage and protected namespace custody.
/// A real directory junction cannot enroll an account namespace or alter its authored target.
#[test]
fn account_namespace_rejects_an_actual_junction_before_creating_children()
-> crate::setup_storage::test_worker::TestResult {
    use super::account::namespace_directory::{Namespace, resolve};
    use std::os::windows::fs::MetadataExt;
    crate::setup_storage::test_worker::run_reserved(8 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let account = temporary.path().join("installation");
        let target = temporary.path().join("authored");
        std::fs::create_dir(&account)?;
        std::fs::create_dir(&target)?;
        std::fs::write(target.join("preserve"), b"authored")?;
        let link = account.join("runtime");
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
/// Runtime records resolve exactly to `<base>\runtime\<leaf>` on the production reservation,
/// creating a missing installation base and reopening the same directory.
#[test]
fn runtime_storage_resolves_below_the_installation_base()
-> crate::setup_storage::test_worker::TestResult {
    crate::setup_storage::test_worker::run_reserved(8 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let base = temporary.path().join("SugarSubstitute");
        let path = super::super::with(context, |owner| {
            owner.runtime_storage(&base, "history", true, context)
        })?;
        let canonical = std::fs::canonicalize(&base)?;
        assert_eq!(std::fs::canonicalize(&path)?, canonical.join("runtime").join("history"));
        let reopened = super::super::with(context, |owner| {
            owner.runtime_storage(&base, "history", false, context)
        })?;
        assert_eq!(reopened, path);
        Ok(())
    })
}
