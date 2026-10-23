// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish desktop startup contracts after SugarCubes configures its model-root owner.
use super::*;
pub(super) fn write(
    root: &Path,
    selection: &Selection,
    runtime: &RuntimePaths,
    cube: &Path,
) -> io::Result<()> {
    let configuration = root.join("startup.local.toml");
    let startup = toml::to_string(&serde_json::json!({
        "schema_version": 1, "mode": "launch", "port": 8188, "profile": "default",
        "locale": selection.locale, "theme": "dark", "reduced_motion": false,
        "launch": {
            "root": runtime.directory, "interpreter": runtime.python,
            "arguments": ["--output-directory", selection.outputs],
        },
    }))
    .map_err(io::Error::other)?;
    files::write_immutable(&configuration, startup.as_bytes(), files::create_new)?;
    let arguments = [
        ("--startup-config", configuration),
        ("--frontend-package", runtime.frontend.clone()),
        ("--frontend-metadata", runtime.metadata.clone()),
        ("--sugarcubes-root", cube.to_owned()),
        ("--backend-script", runtime.directory.join("main.py")),
        ("--profile-directory", selection.install_root.join("desktop-data").join("profile")),
    ]
    .into_iter()
    .flat_map(|(flag, path)| [serde_json::json!(flag), serde_json::json!(path)])
    .collect::<Vec<_>>();
    files::write_immutable(
        &root.join("sugarsubstitute-launch.local.json"),
        &serde_json::to_vec(&serde_json::json!({"schema_version": 1, "arguments": arguments}))
            .map_err(io::Error::other)?,
        files::create_new,
    )
}
