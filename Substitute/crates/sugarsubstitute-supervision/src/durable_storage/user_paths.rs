// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit installation user paths through retained native ancestry and actual account protection.
use super::{
    check,
    files::{self, Directory},
    invalid,
    protected::directories::Mode,
    protection,
};
use std::{
    ffi::OsStr,
    io,
    path::{Path, PathBuf},
};
use sugarsubstitute_contracts::execution::JobContext;

/// Resolve a destination below the actual pinned user directory; strings grant no confinement.
/// Every retained directory from user through the parent must have the same account protection.
pub(crate) fn parent(
    installation: &Directory,
    destination: &Path,
    context: &JobContext,
) -> io::Result<(Directory, PathBuf)> {
    check(context)?;
    files::lexical(destination)?;
    let user = Directory::open(
        &sugarsubstitute_configuration::layout::InstallationLayout::new(installation.path()).user(),
        false,
    )?;
    let parent = Directory::open(
        destination.parent().ok_or_else(|| invalid("User data parent missing"))?,
        false,
    )?
    .resolved()?;
    if !parent.has_ancestor(&user, context)? {
        return Err(invalid("User data destination is outside the pinned user directory"));
    }
    let user_identity = user.identity()?;
    protection::with(context, |owner| {
        let mut below_user = false;
        for pin in parent.native_pins() {
            check(context)?;
            below_user |= files::file_identity(pin)? == user_identity;
            if below_user {
                owner.verify_file(pin)?;
            }
        }
        if !below_user {
            return Err(invalid("User data ancestry was not retained"));
        }
        Ok(())
    })?;
    let destination = parent
        .path()
        .join(destination.file_name().ok_or_else(|| invalid("User data leaf missing"))?);
    Ok((parent, destination))
}

/// Create only a feature-selected fixed chain below user using protected native directories.
/// Each ordinary component is opened without following reparses; existing protection is verified.
pub(crate) fn create_parents(
    installation: &Directory,
    destination: &Path,
    context: &JobContext,
) -> io::Result<Directory> {
    check(context)?;
    files::lexical(destination)?;
    let relative = destination
        .strip_prefix(installation.path())
        .map_err(|_| invalid("User data creation escaped its installation"))?;
    let mut components = relative.components();
    if components.next().map(|part| part.as_os_str()) != Some(OsStr::new("user")) {
        return Err(invalid("User data creation requires the user directory"));
    }
    let mut path = installation.path().join("user");
    protection::with(context, |owner| {
        owner.directory(&path, Mode::OpenOrCreate, context)?;
        for part in components {
            path.push(part);
            owner.directory(&path, Mode::OpenOrCreate, context)?;
        }
        Directory::open(&path, false)?.resolved()
    })
}
