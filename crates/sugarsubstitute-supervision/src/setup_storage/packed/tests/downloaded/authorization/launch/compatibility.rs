// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Historical and future packages cannot accidentally enter the inherited launch protocol.
use super::*;
use crate::setup_storage::ReservedLaunch;
use sugarsubstitute_contracts::application_launch::{CONTRACT_FILE, Rejection};

const FUTURE: &[u8] = br#"{"schema_version":1,"installed_launch_protocols":[2]}"#;

#[test]
fn incompatible_download_cannot_change_accepted_selection_or_begin_preparation() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 96 * 1024 * 1024, |context, _| {
        for (contract, expected) in
            [(None, Rejection::Missing), (Some(FUTURE), Rejection::Unsupported)]
        {
            let temporary = tempfile::tempdir()?;
            let directory = Directory::open(temporary.path(), false)?;
            let (accepted, _) = accept_fixture(&directory, context)?;
            let workspace = tempfile::tempdir()?;
            let (manifest, compressed) = test_package::distribution_with_contract(contract)?;
            let archive = test_package::seal(&manifest, &compressed);
            let signed = SignedPackage::new(&archive, 2)?;
            let trust = TrustStore::open(&directory.path, [92; 32], context)?;
            let target = trust
                .offer(
                    signed.metadata.metadata(),
                    &signed.request,
                    &signed.descriptor,
                    NOW,
                    context,
                )?
                .into_package()?;
            let bundle = Arc::new(VerifiedBundle::from_download(
                receive(&archive, &directory.path, target, context)?,
                workspace.path(),
                context,
            )?);
            let preparation =
                crate::setup_storage::InstallationSession::maintain(&directory.path, context)?
                    .prepare(bundle, context)?;
            let store = SelectionStore::open(&directory.path, context)?;
            let before = store.load(context)?;
            let result = preparation.prepare_update(
                preparation.maintenance_runtime().ok_or("Runtime absent")?,
                &signed.authorization(&trust),
                || NOW,
                context,
            );
            let Err(crate::setup_storage::TrustFailure::Storage(cause)) = result else {
                return Err("Expected explicit launch compatibility rejection".into());
            };
            assert_eq!(
                cause.get_ref().and_then(|cause| cause.downcast_ref::<Rejection>()),
                Some(&expected)
            );
            assert_eq!(store.load(context)?, before);
            let after = store.load(context)?.ok_or("Selection absent")?;
            assert_eq!(after.accepted(), Some(&accepted));
            assert!(after.candidate().is_none());
        }
        Ok(())
    })
}

/// Seed a historical authenticated inventory and its protected digest without claiming a new trial.
fn historical_declaration(
    directory: &Directory,
    generation: &Generation,
    contract: Option<&[u8]>,
) -> TestResult {
    let root = directory.path.join("releases").join(generation.id.as_str());
    let manifest_path = root.join("application-package.json");
    let mut manifest: serde_json::Value = serde_json::from_slice(&std::fs::read(&manifest_path)?)?;
    let entries = manifest["files"].as_array_mut().ok_or("Inventory absent")?;
    let index = entries
        .iter()
        .position(|entry| entry["path"] == format!("app/{CONTRACT_FILE}"))
        .ok_or("Contract absent")?;
    match contract {
        None => {
            entries.remove(index);
            std::fs::remove_file(root.join(CONTRACT_FILE))?;
        }
        Some(bytes) => {
            std::fs::write(root.join(CONTRACT_FILE), bytes)?;
            entries[index]["bytes"] = bytes.len().into();
            entries[index]["sha256"] = format!("{:x}", Sha256::digest(bytes)).into();
        }
    }
    let bytes = serde_json::to_vec(&manifest)?;
    std::fs::write(manifest_path, &bytes)?;
    let record = directory.path.join(".selection-state/.substitute-selection.json");
    let mut state: serde_json::Value = serde_json::from_slice(&std::fs::read(&record)?)?;
    state["accepted"]["package"] = format!("{:x}", Sha256::digest(bytes)).into();
    std::fs::write(record, serde_json::to_vec(&state)?)?;
    Ok(())
}

#[test]
fn historical_direct_launch_remains_available_but_unsupported_transfer_is_rejected() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 96 * 1024 * 1024, |context, _| {
        for (contract, expected) in
            [(None, Rejection::Missing), (Some(FUTURE), Rejection::Unsupported)]
        {
            let temporary = tempfile::tempdir()?;
            let directory = Directory::open(temporary.path(), false)?;
            let (generation, _) = accept_fixture(&directory, context)?;
            historical_declaration(&directory, &generation, contract)?;
            if contract.is_none() {
                // An unlisted sidecar cannot assert capabilities for a historical application.
                std::fs::write(
                    directory
                        .path
                        .join("releases")
                        .join(generation.id.as_str())
                        .join(CONTRACT_FILE),
                    sugarsubstitute_contracts::application_launch::CURRENT_CONTRACT,
                )?;
            }
            let store = Arc::new(SelectionStore::open(&directory.path, context)?);
            let before = store.load(context)?;
            let mut run = InstallationRun::acquire(store.clone(), context)?;
            assert!(matches!(
                run.select_launch(None, context)?,
                InstallationLaunch::Application(_)
            ));
            drop(run);
            let ReservedLaunch::Application(reserved) = store.reserve_launch(context)? else {
                return Err("Application absent".into());
            };
            let Err(cause) = reserved.prepare_inheritance(context) else {
                return Err("Unsupported transfer admitted".into());
            };
            assert_eq!(
                cause.get_ref().and_then(|cause| cause.downcast_ref::<Rejection>()),
                Some(&expected)
            );
            assert_eq!(store.load(context)?, before);
            drop(
                std::fs::OpenOptions::new()
                    .write(true)
                    .open(directory.path.join(".selection-state/.substitute-selection.json"))?,
            );
            assert!(InstallationRun::acquire(store, context).is_ok());
        }
        Ok(())
    })
}

#[test]
fn altered_launch_declaration_cannot_override_authenticated_inventory() -> TestResult {
    worker::run_reserved(TRUST_RESERVATION_BYTES + 96 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let directory = Directory::open(temporary.path(), false)?;
        let (generation, _) = accept_fixture(&directory, context)?;
        let contract =
            directory.path.join("releases").join(generation.id.as_str()).join(CONTRACT_FILE);
        std::fs::write(&contract, FUTURE)?;
        let store = Arc::new(SelectionStore::open(&directory.path, context)?);
        let before = store.load(context)?;
        assert!(store.reserve_launch(context).is_err());
        assert_eq!(store.load(context)?, before);
        assert_eq!(std::fs::read(contract)?, FUTURE);
        Ok(())
    })
}
