// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real signature, archive, native custody and component rejection tests with inert payloads.
use super::*;
use crate::setup_storage::test_package::updater::{archive, evidence, run};
use crate::setup_storage::{
    self as storage, test_package,
    test_worker::{self as worker, TestResult},
};
use std::sync::Arc;
use sugarsubstitute_update::fixture::{EXPIRES, NOW};
#[test]
fn changed_subscription_revokes_retained_updater_evidence() -> TestResult {
    run(|context| {
        let root = worker::installation(96, context)?;
        let workspace = tempfile::tempdir()?;
        let (bytes, _) = archive(
            sugarsubstitute_contracts::application_launch::updater::CURRENT_UPDATER_CONTRACT,
        )?;
        let (trust, offer, download) = evidence(root.path(), &bytes, Component::Updater, context)?;
        let package = UpdaterPackage::import(
            root.path(),
            trust,
            offer,
            download,
            workspace.path(),
            || NOW,
            context,
        )?;
        let preference = package.trust.subscription(context)?;
        package.trust.select_subscription(
            preference,
            sugarsubstitute_contracts::update::Channel::Canary,
            context,
        )?;
        assert!(package.verify_current(root.path(), NOW, context).is_err());
        assert!(!root.path().join("releases").exists());
        assert_eq!(
            std::fs::read(package.payload.root.join("app/sugarsubstitute-updater.exe"))?,
            b"inert engine"
        );
        Ok(())
    })
}

#[test]
fn updater_import_pins_exact_files_without_application_or_setup_lease_collision() -> TestResult {
    run(|context| {
        let root = worker::installation(92, context)?;
        let workspace = tempfile::tempdir()?;
        let store = Arc::new(storage::SelectionStore::open(root.path(), context)?);
        let _run = storage::InstallationRun::acquire(store, context)?;
        let _setup = storage::transaction::admission::Admission::open(root.path(), false)?;
        let (bytes, digest) = archive(
            sugarsubstitute_contracts::application_launch::updater::CURRENT_UPDATER_CONTRACT,
        )?;
        let (trust, offer, download) = evidence(root.path(), &bytes, Component::Updater, context)?;
        let package = UpdaterPackage::import(
            root.path(),
            trust,
            offer,
            download,
            workspace.path(),
            || NOW,
            context,
        )?;
        assert_eq!(package.digest(), digest);
        assert!(!root.path().join("releases").exists());
        let executable = package.payload.root.join("app/sugarsubstitute-updater.exe");
        assert!(std::fs::write(&executable, b"replacement").is_err());
        assert!(std::fs::rename(&executable, executable.with_extension("old")).is_err());
        package.verify_current(root.path(), NOW, context)?;
        assert!(package.verify_current(root.path(), EXPIRES, context).is_err());
        drop(package);
        std::fs::write(&executable, b"released")?;
        Ok(())
    })
}

#[test]
fn foreign_component_and_application_payload_are_rejected_before_extraction() -> TestResult {
    run(|context| {
        let (updater, _) = archive(
            sugarsubstitute_contracts::application_launch::updater::CURRENT_UPDATER_CONTRACT,
        )?;
        let (manifest, compressed) = test_package::distribution()?;
        let app = test_package::seal(&manifest, &compressed);
        for (bytes, component) in [(&updater, Component::Application), (&app, Component::Updater)] {
            let root = worker::installation(93, context)?;
            let workspace = tempfile::tempdir()?;
            let (trust, offer, download) = evidence(root.path(), bytes, component, context)?;
            assert!(
                UpdaterPackage::import(
                    root.path(),
                    trust,
                    offer,
                    download,
                    workspace.path(),
                    || NOW,
                    context
                )
                .is_err()
            );
            assert!(
                !workspace
                    .path()
                    .join(crate::setup_storage::cache::Namespace::PackedPayloads.directory())
                    .exists()
            );
        }
        Ok(())
    })
}

#[test]
fn updater_declaration_clock_and_installation_identity_fail_closed() -> TestResult {
    run(|context| {
        for case in 0..3 {
            let root = worker::installation(94, context)?;
            let foreign = worker::installation(95, context)?;
            let workspace = tempfile::tempdir()?;
            let contract = if case == 0 {
                b"{}".as_slice()
            } else {
                sugarsubstitute_contracts::application_launch::updater::CURRENT_UPDATER_CONTRACT
            };
            let (bytes, _) = archive(contract)?;
            let (trust, offer, download) =
                evidence(root.path(), &bytes, Component::Updater, context)?;
            let mut time = NOW + 1;
            let result = UpdaterPackage::import(
                if case == 2 { foreign.path() } else { root.path() },
                trust,
                offer,
                download,
                workspace.path(),
                || {
                    time -= 1;
                    time
                },
                context,
            );
            assert!(result.is_err());
            if case == 1 {
                assert!(matches!(
                    result,
                    Err(TrustFailure::Verification(sugarsubstitute_update::Error::ClockRollback))
                ));
            }
            assert!(!root.path().join("releases").exists());
            if case == 2 {
                assert!(
                    !workspace
                        .path()
                        .join(crate::setup_storage::cache::Namespace::PackedPayloads.directory())
                        .exists()
                );
            }
        }
        Ok(())
    })
}

#[test]
fn updater_inventory_requires_entry_roles_and_allows_only_authenticated_runtime_libraries()
-> TestResult {
    use sugarsubstitute_update::package::Manifest;
    let roles = [
        "app/sugarsubstitute-updater.exe",
        "app/sugarsubstitute_updater.pdb",
        "app/sugarsubstitute-updater.json",
    ];
    let make = |names: &[&str]| -> Result<Manifest, Box<dyn std::error::Error + Send + Sync>> {
        let files: Vec<_> = names
            .iter()
            .map(|name| serde_json::json!({"path":name,"bytes":0,"sha256":"a".repeat(64)}))
            .collect();
        Ok(Manifest::decode(&serde_json::to_vec(
            &serde_json::json!({"schema_version":1,"release_id":"test","files":files}),
        )?)?)
    };
    storage::inventory::require_component(&make(&roles)?, Component::Updater)?;
    assert!(storage::inventory::require_component(&make(&roles)?, Component::Application).is_err());
    for index in 0..roles.len() {
        let mut missing = roles.to_vec();
        missing.remove(index);
        assert!(
            storage::inventory::require_component(&make(&missing)?, Component::Updater).is_err()
        );
    }
    for (extra, allowed) in [
        ("app/vcruntime140.dll", true),
        ("app/future-runtime.dll", true),
        ("app/sugarsubstitute.exe", false),
        ("app/setup-generation.json", false),
        ("worker/package.json", false),
        ("app/dependencies/library.dll", false),
        ("app/Runtime.DLL", false),
    ] {
        let mut names = roles.to_vec();
        names.push(extra);
        assert_eq!(
            storage::inventory::require_component(&make(&names)?, Component::Updater).is_ok(),
            allowed,
            "{extra}"
        );
    }
    Ok(())
}
