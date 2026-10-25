// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real destination reads and pins for copy completion and launch admission.
use super::*;
use crate::setup_storage::inventory::is_diagnostic_file;
use crate::setup_storage::{materialize, test_package, test_worker};
use sha2::{Digest, Sha256};
use sugarsubstitute_contracts::application_launch::{
    CURRENT_CONTRACT, CURRENT_LAUNCHER_CONTRACT, updater::CURRENT_UPDATER_CONTRACT,
};

/// Materialize authenticated inert roles, including all diagnostic names and an unknown PDB.
fn fixture(root: &Path, context: &JobContext) -> io::Result<(Manifest, std::path::PathBuf)> {
    let (bytes, compressed) = test_package::distribution_with_roles(
        Some(CURRENT_CONTRACT),
        Some(CURRENT_LAUNCHER_CONTRACT),
        Some(CURRENT_UPDATER_CONTRACT),
    )?;
    let source = root.join("setup.exe");
    std::fs::write(&source, test_package::seal(&bytes, &compressed))?;
    let cache = crate::setup_storage::cache::RuntimeCache::open(
        root,
        crate::setup_storage::cache::CachePolicy::default(),
        context,
    )?;
    let bundle = materialize(&source, &cache, &format!("{:x}", Sha256::digest(&bytes)), context)?;
    let extracted = root.join("installed");
    bundle.payload.copy("app/", &extracted.join("app"), context, files::create_new)?;
    drop(bundle);
    let mut value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let entries = value["files"].as_array_mut().ok_or_else(|| invalid("Fixture files absent"))?;
    for name in [
        "sugarsubstitute.pdb",
        "crashpad_handler.pdb",
        "sugarsubstitute_crashpad_client.pdb",
        "unknown.pdb",
        "nested/sugarsubstitute.pdb",
    ] {
        let data = b"inert diagnostic fixture";
        let path = extracted.join("app").join(name);
        std::fs::create_dir_all(path.parent().ok_or_else(|| invalid("Fixture parent absent"))?)?;
        std::fs::write(path, data)?;
        entries.push(serde_json::json!({"path":format!("app/{name}"),"bytes":data.len(),"sha256":format!("{:x}",Sha256::digest(data))}));
    }
    Ok((
        Manifest::decode(&serde_json::to_vec(&value)?).map_err(io::Error::other)?,
        extracted.join("app"),
    ))
}

#[test]
fn launch_does_not_open_missing_diagnostics_but_copy_completion_requires_them()
-> test_worker::TestResult {
    test_worker::run_reserved(48 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let (manifest, directory) = fixture(root.path(), context)?;
        drop(VerifiedRuntime::verify_copy(&manifest, &directory, context)?);
        for entry in manifest.entries() {
            if let Some(name) =
                entry.path().strip_prefix("app/").filter(|name| is_diagnostic_file(name))
            {
                std::fs::remove_file(directory.join(name))?;
            }
        }
        let runtime = VerifiedRuntime::open(&manifest, &directory, context)?;
        runtime.require_installed_launch()?;
        assert!(std::fs::write(directory.join("sugarsubstitute.exe"), b"replacement").is_err());
        assert!(VerifiedRuntime::verify_copy(&manifest, &directory, context).is_err());
        Ok(())
    })
}

#[test]
fn damaged_diagnostics_do_not_block_launch_and_never_pass_copy_verification()
-> test_worker::TestResult {
    test_worker::run_reserved(48 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let (manifest, directory) = fixture(root.path(), context)?;
        let copy = VerifiedRuntime::verify_copy(&manifest, &directory, context)?;
        assert!(std::fs::write(directory.join("sugarsubstitute.pdb"), b"replacement").is_err());
        drop(copy);
        for entry in manifest.entries() {
            if let Some(name) =
                entry.path().strip_prefix("app/").filter(|name| is_diagnostic_file(name))
            {
                std::fs::write(directory.join(name), vec![0; entry.bytes() as usize])?;
            }
        }
        let runtime = VerifiedRuntime::open(&manifest, &directory, context)?;
        runtime.require_installed_launch()?;
        assert!(VerifiedRuntime::verify_copy(&manifest, &directory, context).is_err());
        // Runtime custody must not accidentally retain unverified diagnostic bytes.
        std::fs::write(directory.join("sugarsubstitute.pdb"), b"independent diagnostic consumer")?;
        Ok(())
    })
}

#[test]
fn every_runtime_role_and_unrecognized_symbol_name_remains_authenticated() -> test_worker::TestResult
{
    test_worker::run_reserved(48 * 1024 * 1024, |context, _| {
        let root = tempfile::tempdir()?;
        let (manifest, directory) = fixture(root.path(), context)?;
        for entry in manifest.entries() {
            let Some(name) =
                entry.path().strip_prefix("app/").filter(|name| !is_diagnostic_file(name))
            else {
                continue;
            };
            let path = directory.join(name);
            let original = std::fs::read(&path)?;
            std::fs::write(&path, vec![0; original.len()])?;
            assert!(
                VerifiedRuntime::open(&manifest, &directory, context).is_err(),
                "accepted changed {name}"
            );
            std::fs::write(&path, &original)?;
            drop(VerifiedRuntime::open(&manifest, &directory, context)?);
        }
        Ok(())
    })
}
