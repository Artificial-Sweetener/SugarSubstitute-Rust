// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Export current and separately identified passive history through the same bounded page codec.
use super::*;
#[cfg(test)]
mod tests;

/// Admission for retained export data and sequential source/protection scratch together.
pub const OPERATIONAL_EXPORT_BYTES: u64 = 8 * 1024 * 1024;
const DATA_BYTES: u64 = 2 * 1024 * 1024;

/// Export current pages and passive preserved account/installation history below `base`.
/// Account history has no installation ownership claim and is explicitly labelled as such.
/// The caller reserves `OPERATIONAL_EXPORT_BYTES` on an admitted worker. Original account
/// locations are never opened here; preserved copies are read-only and never get a writer.
/// # Errors
/// Missing all history, malformed/future pages, unknown siblings, links, foreign protection,
/// active-writer contention and genuine job controls remain observable. No destination is granted.
pub fn export(base: &Path, context: &JobContext) -> io::Result<OperationalDiagnosticExport> {
    let memory = context.memory().claim(DATA_BYTES).map_err(io::Error::other)?;
    let mut bytes = Vec::with_capacity(3 * PAGES * PAGE_BYTES);
    let mut found = false;
    protection::with(context, |owner| {
        if let Some(path) = existing(base, DIRECTORY, owner, context)? {
            current(&path, owner, &mut bytes, context)?;
            found = true;
        }
        for (directory, label) in [
            ("account-history", b"history_source=preserved_account\n".as_slice()),
            ("installation-history", b"history_source=preserved_installation\n".as_slice()),
        ] {
            let Some(parent) = existing(base, directory, owner, context)? else {
                continue;
            };
            let root = match Directory::open(&parent.join(DIRECTORY), false) {
                Ok(root) => root,
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => continue,
                Err(cause) => return Err(cause),
            };
            with_closed_source(&root, HistorySource::PreservedCopy, context, |root, _| {
                bytes.extend_from_slice(label);
                project(root, &mut bytes, context)
            })?;
            found = true;
        }
        check(context)
    })?;
    if !found {
        return Err(io::Error::new(io::ErrorKind::NotFound, "Operational history is absent"));
    }
    Ok(OperationalDiagnosticExport { bytes, _memory: memory })
}
/// Observe ordinary absence before entering the native account resolver, whose richer failures
/// retain their exact cause instead of flattening native missing/access/control outcomes together.
fn existing(
    base: &Path,
    leaf: &'static str,
    owner: &mut protection::Protection,
    context: &JobContext,
) -> io::Result<Option<std::path::PathBuf>> {
    let original = match Directory::open(&base.join("runtime").join(leaf), false) {
        Ok(directory) => directory,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(cause) => return Err(cause),
    };
    let path = owner.runtime_storage(base, leaf, false, context)?;
    if Directory::open(&path, false)?.native_identity()? != original.native_identity()? {
        return Err(invalid("Operational export source changed during account admission"));
    }
    check(context)?;
    Ok(Some(path))
}
fn current(
    path: &Path,
    owner: &mut protection::Protection,
    bytes: &mut Vec<u8>,
    context: &JobContext,
) -> io::Result<()> {
    let root = Directory::open(path, false)?.resolved()?;
    let _writer = contention::acquire(owner, &root.path().join("writer"), context)?;
    inventory(&root, owner, context)?;
    if read(&root, "owner", context)? != MARKER {
        return Err(invalid("Operational diagnostic owner changed"));
    }
    project(&root, bytes, context)
}
fn project(root: &Directory, bytes: &mut Vec<u8>, context: &JobContext) -> io::Result<()> {
    let mut pages = pages(root, context)?;
    pages.sort_by_key(|(_, page)| page.sequence);
    for (_, page) in pages {
        if page.omitted != 0 {
            bytes.extend_from_slice(format!("omitted_events={}\n", page.omitted).as_bytes());
        }
        for event in page.events {
            bytes.extend_from_slice(event.as_bytes());
            if !event.ends_with('\n') {
                bytes.push(b'\n');
            }
        }
    }
    check(context)
}
#[cfg(test)]
pub(super) fn export_at(
    path: &Path,
    owner: &mut protection::Protection,
    context: &JobContext,
) -> io::Result<OperationalDiagnosticExport> {
    let memory = context.memory().claim(DATA_BYTES).map_err(io::Error::other)?;
    let mut bytes = Vec::with_capacity(PAGES * PAGE_BYTES);
    current(path, owner, &mut bytes, context)?;
    Ok(OperationalDiagnosticExport { bytes, _memory: memory })
}
