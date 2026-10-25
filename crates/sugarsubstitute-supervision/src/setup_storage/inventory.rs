// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reject undeclared package files before any worker or library can load from the bundle.
use super::invalid;
use crate::durable_storage::files::{self, Directory};
use std::{collections::BTreeSet, io, path::Path};

/// Authenticated package inventory retained under installation ownership, independent of cache.
pub(super) const INSTALLED_MANIFEST: &str = "application-package.json";

/// Validate the requested component before extraction creates any destination files.
/// The component comes from the local delivery owner, never from the archive inventory.
pub(super) fn require_component(
    manifest: &sugarsubstitute_update::package::Manifest,
    component: sugarsubstitute_contracts::update::Component,
) -> io::Result<()> {
    match component {
        sugarsubstitute_contracts::update::Component::Application => require_complete(manifest),
        sugarsubstitute_contracts::update::Component::Updater => {
            // Signed native libraries can evolve with the engine without admitting app or worker roles.
            const ROLES: [&str; 3] = [
                "app/sugarsubstitute-updater.exe",
                "app/sugarsubstitute_updater.pdb",
                "app/sugarsubstitute-updater.json",
            ];
            if ROLES
                .iter()
                .any(|name| !manifest.entries().iter().any(|entry| entry.path() == *name))
                || manifest.entries().iter().any(|entry| {
                    let name = entry.path();
                    !ROLES.contains(&name)
                        && !name.strip_prefix("app/").is_some_and(|leaf| {
                            !leaf.contains('/')
                                && leaf.ends_with(".dll")
                                && leaf == leaf.to_ascii_lowercase()
                        })
                })
            {
                return Err(invalid(
                    "Updater package has missing, unexpected or noncanonical roles",
                ));
            }
            Ok(())
        }
    }
}

/// Windows distribution roles remain with the native adapter; portable manifests own syntax.
/// Both installer and update extraction require the same complete, canonically named payload.
pub(super) fn require_complete(
    manifest: &sugarsubstitute_update::package::Manifest,
) -> io::Result<()> {
    require_roles(
        manifest,
        &[
            "app/sugarsubstitute-launcher.exe",
            "app/sugarsubstitute_launcher.pdb",
            "app/sugarsubstitute-launcher.json",
        ],
    )?;
    require_roles(
        manifest,
        &[
            "app/sugarsubstitute-updater.exe",
            "app/sugarsubstitute_updater.pdb",
            "app/sugarsubstitute-updater.json",
        ],
    )?;
    super::bundle::worker::uses_node(manifest)?;
    require_application(manifest)
}

/// Optional historical roles become mandatory as a complete canonical set when any is present.
fn require_roles(
    manifest: &sugarsubstitute_update::package::Manifest,
    roles: &[&str],
) -> io::Result<()> {
    let present = roles
        .iter()
        .any(|name| manifest.entries().iter().any(|entry| entry.path().eq_ignore_ascii_case(name)));
    if present
        && roles.iter().any(|name| !manifest.entries().iter().any(|entry| entry.path() == *name))
    {
        return Err(invalid(
            "Native executable payload is incomplete or has noncanonical role names",
        ));
    }
    Ok(())
}

fn require_application(manifest: &sugarsubstitute_update::package::Manifest) -> io::Result<()> {
    for entry in manifest.entries() {
        let path = entry.path().to_ascii_lowercase();
        let Some(relative) = path.strip_prefix("app/") else { continue };
        let first = relative.split('/').next().unwrap_or_default();
        if [
            INSTALLED_MANIFEST,
            "application-update-v1.record",
            "installation-authorization-v1.record",
            "setup-generation.json",
            "installation-ready.json",
            "startup.local.toml",
            "sugarsubstitute-launch.local.json",
            "sugarsubstitute.lnk",
            "capture",
        ]
        .contains(&first)
        {
            return Err(invalid("Application payload overlaps installation-owned state"));
        }
    }
    for name in [
        "app/sugarsubstitute.exe",
        "app/sugarsubstitute-release-v1.manifest",
        "app/sugarsubstitute-release-v1.sig",
        "app/crashpad_handler.exe",
        "app/sugarsubstitute_crashpad_client.dll",
        "worker/package.json",
        "worker/out/main/index.cjs",
        "sugarcubes/__init__.py",
        "sugarcubes/pyproject.toml",
        "sugarcubes/prestartup_script.py",
        "sugarcubes/load_model_roots.py",
        "sugarcubes/sugarcubes_model_roots/__init__.py",
    ] {
        if !manifest.entries().iter().any(|entry| entry.path() == name) {
            return Err(invalid(
                "Native distribution is incomplete or has noncanonical role names",
            ));
        }
    }
    Ok(())
}

/// Verify declared names through pinned directories under the original admitted job controls.
/// Features retain the canonical inventory; enumeration grants no payload or deletion authority.
pub(super) fn verify(
    root: &Path,
    files: &BTreeSet<String>,
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> io::Result<()> {
    verify_inventory(root, files, &BTreeSet::new(), context)
}

/// Runtime admission permits absent known symbols while rejecting undeclared loadable content.
pub(super) fn verify_runtime(
    root: &Path,
    manifest: &sugarsubstitute_update::package::Manifest,
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> io::Result<()> {
    let files = manifest.names();
    let optional = files
        .iter()
        .filter(|name| name.strip_prefix("app/").is_some_and(is_diagnostic_file))
        .cloned()
        .collect();
    verify_inventory(root, &files, &optional, context)
}

/// Only fixed native diagnostic roles are deferred; arbitrary extensions remain runtime inputs.
pub(super) fn is_diagnostic_file(relative: &str) -> bool {
    matches!(
        relative,
        "sugarsubstitute.pdb"
            | "sugarsubstitute_launcher.pdb"
            | "sugarsubstitute_updater.pdb"
            | "crashpad_handler.pdb"
            | "sugarsubstitute_crashpad_client.pdb"
    )
}

/// Preserve canonical enumeration and alias rejection for both complete and runtime admission.
fn verify_inventory(
    root: &Path,
    files: &BTreeSet<String>,
    optional: &BTreeSet<String>,
    context: &sugarsubstitute_contracts::execution::JobContext,
) -> io::Result<()> {
    let mut directories = BTreeSet::new();
    for file in files {
        let mut parent = Path::new(file).parent();
        while let Some(path) = parent.filter(|path| !path.as_os_str().is_empty()) {
            directories.insert(path.to_string_lossy().replace('\\', "/"));
            parent = path.parent();
        }
    }
    let mut pending = vec![String::new()];
    let mut observed = BTreeSet::new();
    let mut observed_directories = BTreeSet::new();
    while let Some(relative) = pending.pop() {
        super::check(context)?;
        let directory = Directory::open(&root.join(&relative), false)?;
        for entry in directory.entries(context)? {
            super::check(context)?;
            let entry = entry?;
            let leaf =
                entry.file_name().into_string().map_err(|_| invalid("Non-Unicode bundle entry"))?;
            files::relative(&leaf)?;
            let name = if relative.is_empty() { leaf } else { format!("{relative}/{leaf}") };
            let key = name.to_ascii_lowercase();
            let kind = entry.file_type()?;
            if kind.is_dir() && directories.contains(&key) {
                if !observed_directories.insert(key) {
                    return Err(invalid("Aliased bundle directory"));
                }
                pending.push(name);
            } else if kind.is_file() && (files.contains(&key) || key == "setup-manifest.json") {
                if !observed.insert(key) {
                    return Err(invalid("Aliased bundle entry"));
                }
            } else {
                return Err(invalid("Installer package contains undeclared content"));
            }
        }
    }
    if !files.iter().all(|name| observed.contains(name) || optional.contains(name))
        || !observed.contains("setup-manifest.json")
    {
        return Err(invalid("Installer package files disappeared during verification"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn updater_roles_require_a_complete_canonical_set() -> io::Result<()> {
        use sugarsubstitute_update::package::Manifest;
        let (bytes, _) = crate::setup_storage::test_package::distribution_with_roles(
            Some(sugarsubstitute_contracts::application_launch::CURRENT_CONTRACT),
            None,
            Some(sugarsubstitute_contracts::application_launch::updater::CURRENT_UPDATER_CONTRACT),
        )?;
        require_complete(&Manifest::decode(&bytes).map_err(io::Error::other)?)?;
        for name in [
            "app/sugarsubstitute-updater.exe",
            "app/sugarsubstitute_updater.pdb",
            "app/sugarsubstitute-updater.json",
        ] {
            for remove in [false, true] {
                let mut value: serde_json::Value = serde_json::from_slice(&bytes)?;
                let entries =
                    value["files"].as_array_mut().ok_or_else(|| invalid("Fixture files absent"))?;
                let index = entries
                    .iter()
                    .position(|entry| entry["path"] == name)
                    .ok_or_else(|| invalid("Fixture role absent"))?;
                if remove {
                    entries.remove(index);
                } else {
                    entries[index]["path"] =
                        name.replace("sugarsubstitute", "SugarSubstitute").into();
                }
                assert!(
                    require_complete(
                        &Manifest::decode(&serde_json::to_vec(&value)?)
                            .map_err(io::Error::other)?
                    )
                    .is_err()
                );
            }
        }
        Ok(())
    }

    #[test]
    fn payloads_cannot_occupy_installer_metadata_or_runtime_state_names() -> io::Result<()> {
        use sugarsubstitute_update::package::Manifest;
        let (bytes, _) = crate::setup_storage::test_package::distribution()?;
        for path in [
            "app/application-package.json",
            "app/APPLICATION-UPDATE-V1.RECORD",
            "app/INSTALLATION-AUTHORIZATION-V1.RECORD",
            "app/STARTUP.LOCAL.TOML",
            "app/setup-generation.json/child",
            "app/capture/report",
            "app/SugarSubstitute.lnk",
        ] {
            let mut value: serde_json::Value = serde_json::from_slice(&bytes)?;
            value["files"]
                .as_array_mut()
                .ok_or_else(|| invalid("Fixture files absent"))?
                .push(serde_json::json!({"path":path,"bytes":0,"sha256":"a".repeat(64)}));
            let manifest =
                Manifest::decode(&serde_json::to_vec(&value)?).map_err(io::Error::other)?;
            assert!(require_complete(&manifest).is_err(), "accepted {path}");
        }
        Ok(())
    }
    #[test]
    fn platform_roles_require_all_payloads_with_canonical_names() -> io::Result<()> {
        use sugarsubstitute_update::package::Manifest;
        let (bytes, _) = crate::setup_storage::test_package::distribution()?;
        require_complete(&Manifest::decode(&bytes).map_err(io::Error::other)?)?;
        let mut value: serde_json::Value = serde_json::from_slice(&bytes)?;
        value["files"][0]["path"] = "app/SugarSubstitute.exe".into();
        assert!(
            require_complete(
                &Manifest::decode(&serde_json::to_vec(&value)?).map_err(io::Error::other)?
            )
            .is_err()
        );
        value["files"] = serde_json::json!([]);
        assert!(Manifest::decode(&serde_json::to_vec(&value)?).is_err());
        Ok(())
    }
    #[test]
    fn undeclared_libraries_and_empty_load_directories_are_rejected()
    -> crate::setup_storage::test_worker::TestResult {
        crate::setup_storage::test_worker::run_reserved(4 * 1024 * 1024, |context, _| {
            let root = tempfile::tempdir()?;
            std::fs::create_dir(root.path().join("worker"))?;
            std::fs::write(root.path().join("setup-manifest.json"), b"manifest")?;
            std::fs::write(root.path().join("worker/main.cjs"), b"declared")?;
            let names = BTreeSet::from(["worker/main.cjs".to_owned()]);
            verify(root.path(), &names, context)?;
            let extra = root.path().join("worker/unexpected.dll");
            std::fs::write(&extra, b"untrusted")?;
            assert!(verify(root.path(), &names, context).is_err());
            std::fs::remove_file(&extra)?;
            std::fs::create_dir(root.path().join("worker/undeclared"))?;
            assert!(verify(root.path(), &names, context).is_err());
            assert_eq!(std::fs::read(root.path().join("worker/main.cjs"))?, b"declared");
            Ok(())
        })
    }
}
