// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Copy source-host defaults only under the elected namespace and native released profile.
use super::*;
use crate::durable_storage::{imports, protected::directories::Mode, protection, user_paths};
use crate::setup_storage::user_data::{HistoricalArchiveDisposition, host};
use sugarsubstitute_configuration::layout::InstallationLayout;
use sugarsubstitute_contracts::lifecycle::NamespaceId;

pub(super) fn settle(
    source: &VerifiedHost,
    paths: &StartupPaths,
    correlation: &InstallationId,
    profile: &ProfileId,
    namespace: NamespaceId,
    archives: &mut impl FnMut(&Path, &JobContext) -> io::Result<HistoricalArchiveDisposition>,
    context: &JobContext,
) -> io::Result<PathBuf> {
    let layout = InstallationLayout::new(source.root.path());
    let requested = paths.as_paths()[5];
    let legacy_path = source.root.path().join("desktop-data").join("profile");
    let association = imports::ImportAssociation::SourceHost { correlation, profile, namespace };
    // A complete copy no longer depends on the old profile's current writer or later contents.
    // Pending archive preservation remains a separate obligation with its own source admission.
    if same_default(requested, &legacy_path) {
        match imports::verify_completed(
            &source.root,
            association,
            &layout.webview_profile(),
            context,
        ) {
            Ok(()) if !archives_pending(source, association, context)? => {
                return Ok(layout.webview_profile());
            }
            Ok(()) => {}
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
            Err(cause) => return Err(cause),
        }
    }
    let legacy = host::existing(&legacy_path)?;
    let selected = match (&legacy, host::existing(requested)?) {
        (Some(legacy), Some(requested)) => {
            legacy.native_identity()? == requested.native_identity()?
        }
        _ => same_default(requested, &legacy_path),
    };
    if selected {
        let target = layout.webview_profile();
        user_paths::create_parents(
            &source.root,
            target.parent().ok_or_else(|| invalid("Source profile parent absent"))?,
            context,
        )?;
        let completed = match imports::verify_completed(&source.root, association, &target, context)
        {
            Ok(()) => true,
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => false,
            Err(cause) => return Err(cause),
        };
        if completed && !archives_pending(source, association, context)? {
            return Ok(target);
        }
        if let Some(legacy) = legacy {
            crate::profiles::data_import::with_released(
                &legacy,
                &[namespace],
                context,
                |released| {
                    if !completed {
                        imports::copy_tree(
                            &source.root,
                            association,
                            released.directory(),
                            &target,
                            imports::RootEntries::Except(&released.excluded_root_controls()),
                            context,
                        )?;
                    }
                    // The same actual profile/account/root ownership admits only passive archives.
                    // Document association remains with the native archive owner after this copy.
                    host::import_archives(&source.root, association, archives, context)
                },
            )?;
        } else {
            reject_unowned_archives(source, association, context)?;
            if !completed {
                create_confined(source, &target, context)?;
            }
        }
        return Ok(target);
    }
    if same_default(requested, &layout.webview_profile()) {
        user_paths::create_parents(&source.root, &layout.user().join("desktop-data"), context)?;
    }
    let selected = create_confined(source, requested, context)?;
    if let Some(legacy) = legacy {
        if host::existing(&source.root.path().join("desktop-data").join("input-canvas"))?.is_some()
        {
            crate::profiles::data_import::with_released(&legacy, &[namespace], context, |_| {
                host::import_archives(&source.root, association, archives, context)
            })?;
        }
    } else {
        reject_unowned_archives(source, association, context)?;
    }
    Ok(selected)
}

fn archives_pending(
    source: &VerifiedHost,
    association: imports::ImportAssociation<'_>,
    context: &JobContext,
) -> io::Result<bool> {
    match imports::verify_completed(
        &source.root,
        association,
        &source.root.path().join("user/input-canvas"),
        context,
    ) {
        Ok(()) => Ok(false),
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
            Ok(host::existing(&source.root.path().join("desktop-data/input-canvas"))?.is_some())
        }
        Err(cause) => Err(cause),
    }
}

fn reject_unowned_archives(
    source: &VerifiedHost,
    association: imports::ImportAssociation<'_>,
    context: &JobContext,
) -> io::Result<()> {
    if host::existing(&source.root.path().join("desktop-data").join("input-canvas"))?.is_some() {
        return imports::verify_completed(
            &source.root,
            association,
            &source.root.path().join("user/input-canvas"),
            context,
        );
    }
    Ok(())
}

fn create_confined(
    source: &VerifiedHost,
    requested: &Path,
    context: &JobContext,
) -> io::Result<PathBuf> {
    let (_parent, target) = user_paths::parent(&source.root, requested, context)?;
    protection::with(context, |owner| {
        owner.directory(&target, Mode::OpenOrCreate, context)?;
        Ok(())
    })?;
    check(context)?;
    Ok(target)
}

fn same_default(left: &Path, right: &Path) -> bool {
    left.to_str().zip(right.to_str()).is_some_and(|(left, right)| left.eq_ignore_ascii_case(right))
}

#[cfg(test)]
mod tests;
