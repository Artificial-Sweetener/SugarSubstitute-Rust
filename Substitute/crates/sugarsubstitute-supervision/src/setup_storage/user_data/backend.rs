// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Move only implicit mutable defaults selected by verified standalone native provenance.
use super::{
    ExplicitBackendDirectories,
    host::existing,
    installation::InstallationData,
    runtime::{self, LegacySources},
};
use crate::{
    durable_storage::{check, files, imports, invalid, user_paths},
    setup_storage::BackendStorage,
};
use std::{error::Error, io, path::Path};
use sugarsubstitute_contracts::{
    configuration::{ConnectionConfiguration, LaunchConfiguration, StartupConfiguration},
    execution::JobContext,
};

/// Explicit external/base/storage selections retain their original values and data locations.
pub(super) fn settle<E: Error + Send + Sync + 'static>(
    installation: &InstallationData,
    configuration: StartupConfiguration,
    selected: impl FnOnce(&LaunchConfiguration) -> Result<ExplicitBackendDirectories, E>,
    legacy: &mut LegacySources,
    context: &JobContext,
) -> io::Result<StartupConfiguration> {
    let ConnectionConfiguration::Launch(request) = configuration.connection() else {
        return Ok(configuration);
    };
    let BackendStorage::InstallationOwned(_) =
        installation.backend.select(request.root(), context)?
    else {
        return Ok(configuration);
    };
    let selections = selected(request).map_err(io::Error::other)?;
    if selections.base {
        return Ok(configuration);
    }
    let data = installation.root.path().join("user/comfy-data");
    user_paths::create_parents(&installation.root, &data, context)?;
    let mut arguments = request.arguments().to_vec();
    arguments.extend(["--base-directory".into(), runtime::option_path(data.clone())?]);
    for (selected, leaf) in
        [(selections.user, "user"), (selections.input, "input"), (selections.output, "output")]
    {
        if !selected {
            import_default(installation, request.root(), &data, leaf, legacy, context)?;
        }
    }
    if !selections.temporary {
        // Comfy normally purges its active temp directory. Preserve old artifacts outside that
        // lifecycle instead of allowing a first confined launch to delete the recovery copy.
        let recovery = installation
            .root
            .path()
            .join("user/migration-originals/comfy-temporary")
            .join(installation.identity.id.as_str());
        user_paths::create_parents(
            &installation.root,
            recovery.parent().ok_or_else(|| invalid("Temporary recovery parent absent"))?,
            context,
        )?;
        copy_default(installation, &request.root().join("temp"), &recovery, legacy, context)?;
    }
    if !selections.models {
        // Comfy's supported option builds its own complete category/alias registry. The normal
        // SugarCubes selected-root owner still has precedence over this retained default library.
        arguments.extend([
            "--models-directory".into(),
            runtime::option_path(request.root().join("models"))?,
        ]);
    }
    preserve_model_controls(installation, request.root(), &data, legacy, context)?;
    let launch = LaunchConfiguration::new(
        request.root().to_owned(),
        request.interpreter().to_owned(),
        arguments,
        request.port().get(),
    )
    .map_err(io::Error::other)?;
    check(context)?;
    Ok(StartupConfiguration::new(
        ConnectionConfiguration::Launch(launch),
        configuration.profile().clone(),
        configuration.presentation(),
    )
    .with_canvas_renderer(configuration.canvas_renderer().clone()))
}

fn import_default(
    installation: &InstallationData,
    source: &Path,
    data: &Path,
    leaf: &str,
    legacy: &mut LegacySources,
    context: &JobContext,
) -> io::Result<()> {
    copy_default(installation, &source.join(leaf), &data.join(leaf), legacy, context)
}
fn copy_default(
    installation: &InstallationData,
    source: &Path,
    target: &Path,
    legacy: &mut LegacySources,
    context: &JobContext,
) -> io::Result<()> {
    if runtime::completed(installation, target, context)? {
        return Ok(());
    }
    let Some(source) = existing(source)? else {
        return Ok(());
    };
    legacy.require_closed(context)?;
    imports::copy_tree(
        &installation.root,
        imports::ImportAssociation::Installed(installation.snapshot.installation()),
        &source,
        target,
        imports::RootEntries::Except(&[]),
        context,
    )
}

/// Transfer only the two owner-defined original control names, never model assets or other data.
/// Their public SugarCubes codec remains authoritative; this operation preserves raw bytes.
fn preserve_model_controls(
    installation: &InstallationData,
    source: &Path,
    data: &Path,
    legacy: &mut LegacySources,
    context: &JobContext,
) -> io::Result<()> {
    let target = data.join(".substitute");
    if runtime::completed(installation, &target, context)? {
        return Ok(());
    }
    let Some(source) = existing(&source.join(".substitute"))? else {
        return Ok(());
    };
    // Presence of current is authoritative, even if its bytes will later be rejected by its
    // owner. Never inspect or admit an ignored legacy record in place of a present current.
    let selected = ["model_root.json", "managed_model_root.json"]
        .into_iter()
        .find_map(|name| match source.read(name) {
            Ok(file) => Some(Ok((name, file))),
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => None,
            Err(cause) => Some(Err(cause)),
        })
        .transpose()?;
    let Some((name, original)) = selected else {
        return Ok(());
    };
    files::single_link(&original)?;
    if files::length(&original, context)? > 256 * 1024 {
        return Err(invalid("Historical model control exceeds owner admission"));
    }
    legacy.require_closed(context)?;
    let result = imports::copy_tree(
        &installation.root,
        imports::ImportAssociation::Installed(installation.snapshot.installation()),
        &source,
        &target,
        imports::RootEntries::OnlyFiles(&[name]),
        context,
    );
    drop(original);
    result
}
