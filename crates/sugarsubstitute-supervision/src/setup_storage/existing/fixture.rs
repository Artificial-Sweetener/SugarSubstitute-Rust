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
    files::write_new(&root.path.join(ownership::MARKER), ownership::CONTENT)?;
    let generation = root.path.join("releases").join("installed");
    let _directory = Directory::open(&generation, true)?;
    files::write_new(&generation.join("sugarsubstitute.exe"), b"unexecuted fixture")?;
    files::write_new(
        &generation.join("startup.local.toml"),
        b"schema_version = 1\r\n# User customization\r\n[backend]\r\nport = 9191\r\n",
    )?;
    let runtime = RuntimePaths {
        directory: root.path.join("external-comfy"),
        python: root.path.join("python.exe"),
        frontend: root.path.join("frontend"),
        metadata: root.path.join("METADATA"),
    };
    let selection = Selection {
        locale: "ja".into(),
        install_root: root.path.clone(),
        mode: "attached".into(),
        comfy_root: runtime.directory.clone(),
        python: String::new(),
        models: root.path.join("models"),
        outputs: root.path.join("outputs"),
        selection_token: None,
    };
    files::write_new(
        &generation.join("installation-ready.json"),
        &serde_json::to_vec(&serde_json::json!({
            "schema_version": 1, "bundle_sha256": "a".repeat(64), "selection": selection, "runtime": runtime,
        }))?,
    )?;
    let arguments = [
        ("--startup-config", generation.join("startup.local.toml")),
        ("--frontend-package", runtime.frontend),
        ("--frontend-metadata", runtime.metadata),
        ("--sugarcubes-root", root.path.join("custom cube")),
        ("--backend-script", runtime.directory.join("main.py")),
        ("--profile-directory", root.path.join("custom profile")),
    ]
    .into_iter()
    .flat_map(|(flag, path)| [flag.to_owned(), path.to_string_lossy().into_owned()])
    .collect();
    files::write_new(
        &generation.join("sugarsubstitute-launch.local.json"),
        &serde_json::to_vec(&LaunchDescriptor { schema_version: 1, arguments })?,
    )?;
    entrypoint::stage(&generation)?;
    entrypoint::publish(root, &generation, context)?;
    Ok(generation)
}
