// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Authenticate and retain paired launch payloads through actual archive and copy boundaries.
use super::*;
use crate::setup_storage::invalid;
use sugarsubstitute_contracts::application_launch::{CURRENT_CONTRACT, CURRENT_LAUNCHER_CONTRACT};
use sugarsubstitute_update::package::Manifest;

#[test]
fn launcher_payload_survives_authenticated_extraction_and_destination_verification()
-> test_worker::TestResult {
    test_worker::run_reserved(48 * 1024 * 1024, |context, _| {
        let temporary = tempfile::tempdir()?;
        let (manifest, compressed) = test_package::distribution_with_launcher(
            Some(CURRENT_CONTRACT),
            Some(CURRENT_LAUNCHER_CONTRACT),
        )?;
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
        bundle.payload.copy("app/", &destination, context, files::create_new)?;
        let pinned =
            VerifiedRuntime::verify_copy(&bundle.payload.inventory, &destination, context)?;
        pinned.require_installed_launch()?;
        for name in [
            "sugarsubstitute-launcher.exe",
            "sugarsubstitute_launcher.pdb",
            "sugarsubstitute-launcher.json",
        ] {
            let target = destination.join(name);
            assert_eq!(
                std::fs::read(&target)?,
                std::fs::read(bundle.payload.root.join("app").join(name))?
            );
            assert!(std::fs::write(&target, b"replacement").is_err());
        }
        drop(pinned);
        std::fs::write(destination.join("sugarsubstitute-launcher.json"), b"changed contract")?;
        assert!(
            VerifiedRuntime::verify_copy(&bundle.payload.inventory, &destination, context).is_err()
        );
        Ok(())
    })
}

#[test]
fn authenticated_but_incompatible_pairs_do_not_become_verified_bundles() -> test_worker::TestResult
{
    test_worker::run_reserved(32 * 1024 * 1024, |context, _| {
        for (application, launcher, expected) in [
            (None, CURRENT_LAUNCHER_CONTRACT, "Missing"),
            (
                Some(CURRENT_CONTRACT),
                br#"{"schema_version":1,"installation_schemas":[3],"installed_launch_protocol":2}"#
                    .as_slice(),
                "Unsupported",
            ),
            (
                Some(CURRENT_CONTRACT),
                br#"{"schema_version":1,"installation_schemas":[4],"installed_launch_protocol":1}"#
                    .as_slice(),
                "InstallationSchema",
            ),
            (
                Some(CURRENT_CONTRACT),
                br#"{"schema_version":2,"installation_schemas":[3],"installed_launch_protocol":1}"#
                    .as_slice(),
                "Format",
            ),
        ] {
            let temporary = tempfile::tempdir()?;
            let (manifest, compressed) =
                test_package::distribution_with_launcher(application, Some(launcher))?;
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
                return Err("Incompatible launcher pair was admitted".into());
            };
            let rejection = cause
                .get_ref()
                .and_then(|cause| {
                    cause.downcast_ref::<sugarsubstitute_contracts::application_launch::Rejection>()
                })
                .ok_or("Compatibility error cause absent")?;
            assert_eq!(format!("{rejection:?}"), expected);
            assert_eq!(
                std::fs::read(test_worker::cache_file(
                    temporary.path(),
                    crate::setup_storage::cache::Namespace::PackedPayloads,
                    "app/sugarsubstitute-launcher.json"
                )?)?,
                launcher
            );
        }
        Ok(())
    })
}

#[test]
fn incomplete_or_aliased_launcher_roles_are_rejected_before_extraction() -> io::Result<()> {
    let (manifest, _) = test_package::distribution_with_launcher(
        Some(CURRENT_CONTRACT),
        Some(CURRENT_LAUNCHER_CONTRACT),
    )?;
    let original: serde_json::Value = serde_json::from_slice(&manifest)?;
    for name in [
        "app/sugarsubstitute-launcher.exe",
        "app/sugarsubstitute_launcher.pdb",
        "app/sugarsubstitute-launcher.json",
    ] {
        for rename in [false, true] {
            let mut changed = original.clone();
            let entries =
                changed["files"].as_array_mut().ok_or_else(|| invalid("Fixture files absent"))?;
            if rename {
                let entry = entries
                    .iter_mut()
                    .find(|entry| entry["path"] == name)
                    .ok_or_else(|| invalid("Fixture launcher absent"))?;
                entry["path"] = name.replace("sugarsubstitute", "SugarSubstitute").into();
            } else {
                entries.retain(|entry| entry["path"] != name);
            }
            let inventory =
                Manifest::decode(&serde_json::to_vec(&changed)?).map_err(io::Error::other)?;
            assert!(crate::setup_storage::inventory::require_complete(&inventory).is_err());
        }
    }
    Ok(())
}
