// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Settle desktop profiles and native archive containers without copying lifetime authority.
use super::{HistoricalArchiveDisposition, installation::InstallationData};
use crate::durable_storage::{
    check, files::Directory, imports, invalid, protected::directories::Mode, protection, user_paths,
};
use std::{
    io,
    os::windows::fs::MetadataExt,
    path::{Path, PathBuf},
};
use sugarsubstitute_configuration::{layout::InstallationLayout, paths::StartupPaths};
use sugarsubstitute_contracts::execution::JobContext;

/// Archive format/association remain with the native archive owner supplied by composition.
pub(super) fn settle(
    installation: &InstallationData,
    paths: StartupPaths,
    archives: &mut impl FnMut(&Path, &JobContext) -> io::Result<HistoricalArchiveDisposition>,
    context: &JobContext,
) -> io::Result<StartupPaths> {
    import_archives(
        &installation.root,
        imports::ImportAssociation::Installed(installation.snapshot.installation()),
        archives,
        context,
    )?;
    let [configuration, package, metadata, cubes, script, profile] = paths.into_paths();
    let profile = settle_profile(installation, &profile, context)?;
    StartupPaths::new([configuration, package, metadata, cubes, script, profile])
        .map_err(io::Error::other)
}

fn settle_profile(
    installation: &InstallationData,
    requested: &Path,
    context: &JobContext,
) -> io::Result<PathBuf> {
    let layout = InstallationLayout::new(installation.root.path());
    let legacy_path = installation.root.path().join("desktop-data").join("profile");
    let legacy = existing(&legacy_path)?;
    let selected = match (&legacy, existing(requested)?) {
        (Some(legacy), Some(requested)) => {
            legacy.native_identity()? == requested.native_identity()?
        }
        _ => same_default(requested, &legacy_path),
    };
    if !selected {
        if same_default(requested, &layout.webview_profile()) {
            user_paths::create_parents(
                &installation.root,
                &layout.user().join("desktop-data"),
                context,
            )?;
        }
        return confined_profile(installation, requested, context);
    }
    let target = layout.webview_profile();
    user_paths::create_parents(
        &installation.root,
        target.parent().ok_or_else(|| invalid("Profile parent absent"))?,
        context,
    )?;
    match imports::verify_completed(
        &installation.root,
        imports::ImportAssociation::Installed(installation.snapshot.installation()),
        &target,
        context,
    ) {
        Ok(()) => return Ok(target),
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
        Err(cause) => return Err(cause),
    }
    if let Some(source) = legacy {
        installation.with_namespaces(context, |namespaces| {
            crate::profiles::data_import::with_released(&source, namespaces, context, |source| {
                tracing::info!(operation="historical_profile_source_admitted", namespace=?source.namespace(),
                    "Historical profile owner and original exclusions verified");
                imports::copy_tree(
                    &installation.root,
                    imports::ImportAssociation::Installed(installation.snapshot.installation()),
                    source.directory(),
                    &target,
                    imports::RootEntries::Except(&source.excluded_root_controls()),
                    context,
                )
            })
        })?;
        Ok(target)
    } else {
        confined_profile(installation, &target, context)
    }
}

/// Native user ancestry authorizes host storage; explicit outside selections remain untouched errors.
fn confined_profile(
    installation: &InstallationData,
    requested: &Path,
    context: &JobContext,
) -> io::Result<PathBuf> {
    let (_parent, target) = user_paths::parent(&installation.root, requested, context)?;
    protection::with(context, |owner| {
        owner.directory(&target, Mode::OpenOrCreate, context)?;
        Ok(())
    })?;
    check(context)?;
    Ok(target)
}

pub(super) fn import_archives(
    root: &Directory,
    association: imports::ImportAssociation<'_>,
    archives: &mut impl FnMut(&Path, &JobContext) -> io::Result<HistoricalArchiveDisposition>,
    context: &JobContext,
) -> io::Result<()> {
    let target = root.path().join("user").join("input-canvas");
    match imports::verify_completed(root, association, &target, context) {
        Ok(()) => return Ok(()),
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
        Err(cause) => return Err(cause),
    }
    let Some(source) = existing(&root.path().join("desktop-data").join("input-canvas"))? else {
        return Ok(());
    };
    let mut inspected = 0;
    let mut unassociated = 0;
    for entry in source.entries(context)? {
        let entry = entry?;
        inspected += 1;
        if inspected > 8192 {
            return Err(invalid("Historical archive inventory exceeds admission"));
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| invalid("Historical archive name is not Unicode"))?;
        let recognized =
            matches!(name.as_str(), "input-documents.ssdoc" | "input-documents.previous.ssdoc")
                || name
                    .strip_prefix("input-documents.rejected-")
                    .and_then(|name| name.strip_suffix(".ssdoc"))
                    .is_some_and(|digest| {
                        digest.len() == 64
                            && digest
                                .bytes()
                                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                    });
        if !recognized {
            unassociated += 1;
            continue;
        }
        let metadata = entry.current_metadata()?;
        if !metadata.is_file()
            || metadata.file_attributes() & 0x400 != 0
            || metadata.len() > 4 * 1024 * 1024 * 1024
        {
            return Err(invalid("Historical native archive exceeds its ordinary-file admission"));
        }
        let disposition = archives(&entry.path(), context)?;
        tracing::info!(
            operation = "historical_input_archive_inspected",
            ?disposition,
            "Native archive owner classified preserved original bytes"
        );
    }
    tracing::info!(
        operation = "historical_input_container_preserved",
        unassociated,
        "Unassociated original entries retain no workflow or session authority"
    );
    imports::copy_tree(
        root,
        association,
        &source,
        &target,
        imports::RootEntries::Except(&[]),
        context,
    )
}

/// Only fixed verified default spellings are considered when the old directory does not exist.
fn same_default(left: &Path, right: &Path) -> bool {
    left.to_str().zip(right.to_str()).is_some_and(|(left, right)| left.eq_ignore_ascii_case(right))
}
pub(super) fn existing(path: &Path) -> io::Result<Option<Directory>> {
    match Directory::open(path, false) {
        Ok(directory) => Ok(Some(directory)),
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(cause) => Err(cause),
    }
}
