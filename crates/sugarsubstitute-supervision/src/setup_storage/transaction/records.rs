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
    let startup = sugarsubstitute_configuration::initial::managed(
        &runtime.directory,
        &runtime.python,
        &selection.outputs,
        &selection.locale,
    )?;
    files::write_immutable(&configuration, startup.as_bytes(), files::create_new)?;
    let paths = sugarsubstitute_configuration::paths::StartupPaths::new([
        configuration,
        runtime.frontend.clone(),
        runtime.metadata.clone(),
        cube.to_owned(),
        runtime.directory.join("main.py"),
        selection.install_root.join("desktop-data").join("profile"),
    ])
    .map_err(io::Error::other)?;
    files::write_immutable(
        &root.join("sugarsubstitute-launch.local.json"),
        &sugarsubstitute_configuration::descriptor::LaunchDescriptor::encode(paths)?,
        files::create_new,
    )
}

#[cfg(test)]
mod tests;
