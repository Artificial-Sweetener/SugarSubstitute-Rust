// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit passive historical copies without retiring old stages or creating writer controls.
use super::*;
use crate::durable_storage::{
    files::ExistingAccess,
    imports::{self, ImportAssociation, RootEntries},
    protected::directories::Mode,
    user_paths,
};
use std::{fs::File, os::windows::fs::MetadataExt};
#[cfg(test)]
mod tests;

/// Distinguish actual original writer custody from a passive preserved copy's read-only lifecycle.
#[derive(Clone, Copy)]
pub(crate) enum HistorySource {
    /// Existing original history requires its real writer exclusion; it is never created here.
    Original,
    /// A passive preserved copy contains no active writer or process ownership controls.
    PreservedCopy,
}

/// Read only complete admitted pages under the supplied original or passive-copy lifecycle.
/// The caller derives original locations from actual account/installation authority. These
/// bytes never activate a writer or claim; interrupted known stages remain untouched.
/// # Errors
/// Rejects live original writers, writer controls in passive copies, foreign protection,
/// links, unknown/future pages and excessive inventories without repairing any source.
pub(crate) fn with_closed_source<T>(
    root: &Directory,
    source: HistorySource,
    context: &JobContext,
    copy: impl FnOnce(&Directory, &[&str]) -> io::Result<T>,
) -> io::Result<T> {
    check(context)?;
    let _memory = context.memory().claim(2 * 1024 * 1024).map_err(io::Error::other)?;
    protection::with(context, |owner| {
        owner.directory(root.path(), Mode::Existing, context)?;
        let mut pins: Vec<File> = Vec::new();
        if matches!(source, HistorySource::Original) {
            let writer = root.open_existing("writer", ExistingAccess::ExclusiveRead, context)?;
            files::single_link(&writer)?;
            owner.verify_file(&writer)?;
            if files::length(&writer, context)? != 0 {
                return Err(invalid("Historical operational writer control is not empty"));
            }
            pins.push(writer);
        }
        let mut stages = 0;
        let mut visited = 0;
        for entry in root.entries(context)? {
            let entry = entry?;
            visited += 1;
            if visited > PAGES + 2 + 256 {
                return Err(invalid("Historical operational inventory exceeds admission"));
            }
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| invalid("Historical operational name is not Unicode"))?;
            let metadata = entry.current_metadata()?;
            if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0 {
                return Err(invalid("Historical operational entries must be ordinary files"));
            }
            if name == "writer" {
                if matches!(source, HistorySource::PreservedCopy) {
                    return Err(invalid("Passive preserved history cannot activate a writer"));
                }
                continue;
            }
            if interrupted_page_stage(&name) {
                stages += 1;
                if stages > 256 {
                    return Err(invalid(
                        "Historical operational stage inventory exceeds admission",
                    ));
                }
            } else if name != "owner" && !(0..PAGES).any(|slot| name == format!("page-{slot}.json"))
            {
                return Err(invalid("Unknown historical operational bytes are preserved"));
            }
            let file = root.read(&name)?;
            files::single_link(&file)?;
            owner.verify_file(&file)?;
            if files::length(&file, context)? > PAGE_BYTES as u64 {
                return Err(invalid("Historical operational entry exceeds page admission"));
            }
            pins.try_reserve(1).map_err(io::Error::other)?;
            pins.push(file);
        }
        if read(root, "owner", context)? != MARKER {
            return Err(invalid("Historical operational owner changed"));
        }
        drop(pages(root, context)?);
        let result = copy(root, &["writer"])?;
        check(context)?;
        drop(pins);
        Ok(result)
    })
}

/// Seed a complete earlier operational history below `parent` for migration qualification.
#[cfg(test)]
pub(crate) fn seed_earlier_history(parent: &Path, context: &JobContext) -> io::Result<()> {
    protection::with(context, |owner| {
        let path = parent.join(DIRECTORY);
        owner.directory(&path, Mode::CreateNew, context)?;
        append_at(
            &path,
            &["operation=earlier
"
            .into()],
            0,
            owner,
            context,
        )
    })
}

/// Copy an earlier operational history into `user/runtime/<provenance>` for this installation.
/// `source_runtime` is the earlier `runtime` folder the caller derived from actual account or
/// installation authority; an absent history imports nothing and a completed import is verified
/// instead of repeated. The original pages and their writer control are never modified.
/// # Errors
/// Preserves the causes of [`with_closed_source`] and of the shared import owner, including a
/// live original writer, a conflicting destination or an association that changed.
pub(crate) fn import_original(
    installation: &Directory,
    association: ImportAssociation<'_>,
    source_runtime: &Path,
    provenance: &str,
    context: &JobContext,
) -> io::Result<()> {
    let parent = installation.path().join("user/runtime").join(provenance);
    let target = parent.join(DIRECTORY);
    user_paths::create_parents(installation, &parent, context)?;
    match imports::verify_completed(installation, association, &target, context) {
        Ok(()) => return Ok(()),
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {}
        Err(cause) => return Err(cause),
    }
    let source = match Directory::open(&source_runtime.join(DIRECTORY), false) {
        Ok(source) => source,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(cause) => return Err(cause),
    };
    with_closed_source(&source, HistorySource::Original, context, |source, excluded| {
        imports::copy_tree(
            installation,
            association,
            source,
            &target,
            RootEntries::Except(excluded),
            context,
        )
    })
}
