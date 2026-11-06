// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native installation fixture with intentionally external, unprovisioned Comfy paths.
use super::*;
use crate::setup_storage::ownership;
use std::path::PathBuf;
pub(in crate::setup_storage) fn installed(
    root: &Directory,
    context: &JobContext,
) -> io::Result<PathBuf> {
    ownership::establish(root)?;
    let generation = root.path().join("releases").join("installed");
    let _directory = Directory::open(&generation, true)?;
    files::write_new(&generation.join("sugarsubstitute.exe"), b"unexecuted fixture")?;
    files::write_new(
        &generation.join("startup.local.toml"),
        b"schema_version = 1\r\n# User customization\r\n[backend]\r\nport = 9191\r\n",
    )?;
    let runtime = RuntimePaths {
        directory: root.path().join("external-comfy"),
        python: root.path().join("python.exe"),
        frontend: root.path().join("frontend"),
        metadata: root.path().join("METADATA"),
    };
    let selection = Selection {
        locale: "ja".into(),
        install_root: root.path().to_path_buf(),
        mode: "attached".into(),
        comfy_root: runtime.directory.clone(),
        python: String::new(),
        models: root.path().join("models"),
        outputs: root.path().join("outputs"),
        selection_token: None,
    };
    files::write_new(
        &generation.join("installation-ready.json"),
        &serde_json::to_vec(&serde_json::json!({
            "schema_version": 1, "bundle_sha256": "a".repeat(64), "selection": selection, "runtime": runtime,
        }))?,
    )?;
    let paths = sugarsubstitute_configuration::paths::StartupPaths::new([
        generation.join("startup.local.toml"),
        runtime.frontend,
        runtime.metadata,
        root.path().join("custom cube"),
        runtime.directory.join("main.py"),
        root.path().join("custom profile"),
    ])
    .map_err(io::Error::other)?;
    files::write_new(
        &generation.join("sugarsubstitute-launch.local.json"),
        &LaunchDescriptor::encode(paths)?,
    )?;
    entrypoint::stage(&generation)?;
    entrypoint::publish(root, &generation, context)?;
    Ok(generation)
}
