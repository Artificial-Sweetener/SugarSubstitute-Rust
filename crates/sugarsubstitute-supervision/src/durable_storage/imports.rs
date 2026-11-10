// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Copy closed, feature-verified historical trees before activating confined data.
//!
//! Original data and interrupted attempts remain intact. Complete journals and receipts use
//! atomic sibling publication; an incomplete record never occupies an activation-record name.
//! Features retain source writer exclusion and validate their own data schemas before calling.
mod preserved;
mod record;
pub(crate) use preserved::{PreservedTree, retain_preserved_tree};
#[cfg(test)]
mod tests;
mod tree;
use super::{
    check,
    files::{self, Directory},
    invalid, protection, record_publication, staging, user_paths,
};
use record::{Journal, Receipt};
use sha2::{Digest, Sha256};
use std::{fs::File, io, path::Path};
use sugarsubstitute_contracts::{
    configuration::ProfileId, execution::JobContext, installation::InstallationId,
    lifecycle::NamespaceId,
};

/// Feature-verified correlation for a passive import, never a transferable native capability.
/// Source-host correlation is profile/namespace scoped and grants no installation or Comfy ownership.
#[derive(Clone, Copy)]
pub(crate) enum ImportAssociation<'a> {
    /// Actual protected selection identity retained by the installed launch owner.
    Installed(&'a InstallationId),
    /// The elected source host's protected correlation and exact selected profile/namespace.
    SourceHost { correlation: &'a InstallationId, profile: &'a ProfileId, namespace: NamespaceId },
}

/// Fund bounded names, native pins, streaming buffers and repeated validation together.
pub(crate) const WORKSPACE_BYTES: u64 = 12 * 1024 * 1024;

/// A feature's exact root selection; descendants of an admitted directory retain all bytes.
#[derive(Clone, Copy)]
pub(crate) enum RootEntries<'a> {
    /// Preserve the whole tree except independently retained root lifetime controls.
    Except(&'a [&'a str]),
    /// Preserve only exact ordinary root files; no discovered sibling or asset is admitted.
    OnlyFiles(&'a [&'a str]),
}
impl RootEntries<'_> {
    fn includes(self, name: &str) -> bool {
        match self {
            Self::Except(names) => !names.iter().any(|entry| name.eq_ignore_ascii_case(entry)),
            Self::OnlyFiles(names) => names.iter().any(|entry| name.eq_ignore_ascii_case(entry)),
        }
    }
}

/// Copy only a source already admitted by its feature's ownership/schema/lifecycle owner.
/// The supplied association must come from its feature's actual installed/source-host owner.
/// This owner grants neither legacy-file cleanup nor process/claim activation authority.
/// # Errors
/// Preserves native/control causes and rejects links, aliases, changed sources, unsupported
/// receipts and conflicting destinations. Retry keeps every original and unpublished attempt.
pub(crate) fn copy_tree(
    installation: &Directory,
    association: ImportAssociation<'_>,
    source: &Directory,
    destination: &Path,
    selection: RootEntries<'_>,
    context: &JobContext,
) -> io::Result<()> {
    check(context)?;
    let _memory = context.memory().claim(WORKSPACE_BYTES).map_err(io::Error::other)?;
    let (parent, destination) = user_paths::parent(installation, destination, context)?;
    if parent.has_ancestor(source, context)? {
        return Err(invalid(
            "Import requires a distinct destination below the pinned user directory",
        ));
    }
    let expected = Journal::new(
        association,
        installation.identity()?,
        source.identity()?,
        parent.identity()?,
        &destination,
    )?;
    let records = records(installation, true, context)?;
    let _writer = protection::with(context, |owner| {
        owner.exclusive_rendezvous(&records.path().join("writer"), context)
    })?;
    let receipt_name = receipt_name(&destination)?;
    if let Some((receipt, _file)) = read_receipt(&records, &receipt_name, context)? {
        if receipt.journal != expected {
            return Err(invalid("Historical source or installation changed; copies are preserved"));
        }
        let _journal = read_journal(&records, &receipt, context)?;
        return activate(source, &parent, &destination, selection, &receipt, context);
    }
    if open_optional(&destination)?.is_some() {
        return Err(invalid("Existing data has no import receipt; original data is preserved"));
    }
    let stage_path = staging::unique_path(&parent, ".user-import")?;
    let stage_name = stage_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid("Import stage name missing"))?
        .to_owned();
    record_publication::publish(
        &records,
        &format!("{stage_name}.json"),
        &serde_json::to_vec(&expected).map_err(io::Error::other)?,
        record::MAX_RECORD_BYTES,
        context,
    )?;
    protection::with(context, |owner| {
        owner.directory(&stage_path, super::protected::directories::Mode::CreateNew, context)?;
        Ok(())
    })?;
    let stage = Directory::open(&stage_path, false)?;
    let copied = tree::copy(source, &stage, selection, context)?;
    if copied.tree != tree::observe(source, selection, context)?.tree
        || !copied
            .tree
            .same_contents(&tree::observe(&stage, RootEntries::Except(&[]), context)?.tree)
        || source.identity()? != expected.source
    {
        return Err(invalid(
            "Historical data changed during import; original and stage are preserved",
        ));
    }
    let receipt = Receipt {
        journal: expected,
        tree: copied.tree,
        stage: stage_name,
        stage_identity: stage.identity()?,
    };
    drop(stage);
    let bytes = serde_json::to_vec(&receipt).map_err(io::Error::other)?;
    Receipt::decode(&bytes)?;
    record_publication::publish(
        &records,
        &receipt_name,
        &bytes,
        record::MAX_RECORD_BYTES,
        context,
    )?;
    // The copied source pins remain live through publication, independently of digest fields.
    let result = activate(source, &parent, &destination, selection, &receipt, context);
    drop(copied.pins);
    result
}

/// Validate a completed association before a feature uses imported mutable data normally.
/// Later legitimate edits are retained; an import never rewrites an already activated tree.
pub(crate) fn verify_completed(
    installation: &Directory,
    association: ImportAssociation<'_>,
    destination: &Path,
    context: &JobContext,
) -> io::Result<()> {
    check(context)?;
    let _memory = context.memory().claim(2 * 1024 * 1024).map_err(io::Error::other)?;
    let (parent, destination) = user_paths::parent(installation, destination, context)?;
    let records = records(installation, false, context)?;
    let (receipt, _file) = read_receipt(&records, &receipt_name(&destination)?, context)?
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "Historical import receipt missing")
        })?;
    let _journal = read_journal(&records, &receipt, context)?;
    let active = Directory::open(&destination, false)?;
    if !receipt.journal.belongs_to(
        association,
        installation.identity()?,
        parent.identity()?,
        &destination,
    ) || receipt.stage_identity != active.identity()?
    {
        return Err(invalid("Historical import destination association changed"));
    }
    check(context)
}

fn activate(
    source: &Directory,
    parent: &Directory,
    destination: &Path,
    selection: RootEntries<'_>,
    receipt: &Receipt,
    context: &JobContext,
) -> io::Result<()> {
    if let Some(active) = open_optional(destination)? {
        if active.identity()? != receipt.stage_identity {
            return Err(invalid("Imported destination changed; all data is preserved"));
        }
        return check(context);
    }
    let stage = parent.child(&receipt.stage)?;
    let original = tree::observe(source, selection, context)?;
    if stage.identity()? != receipt.stage_identity
        || original.tree != receipt.tree
        || !tree::observe(&stage, RootEntries::Except(&[]), context)?
            .tree
            .same_contents(&receipt.tree)
    {
        return Err(invalid("Interrupted import changed; original and stage are preserved"));
    }
    let path = stage.path().to_owned();
    drop(stage);
    let result = protection::with(context, |owner| {
        owner.publish(
            &path,
            destination,
            receipt.stage_identity,
            protection::Publication::Directory,
            context,
        )
    });
    drop(original);
    result?;
    check(context)
}

fn records(installation: &Directory, create: bool, context: &JobContext) -> io::Result<Directory> {
    let path = installation.path().join("user/migration-originals/import-records");
    if create {
        user_paths::create_parents(installation, &path, context)
    } else {
        let (_parent, path) = user_paths::parent(installation, &path, context)?;
        // Nothing was ever recorded when the folder is absent. Report that as a plain NotFound
        // here; inside the protected owner it would surface as an opaque native failure.
        drop(Directory::open(&path, false)?);
        protection::with(context, |owner| {
            owner.directory(&path, super::protected::directories::Mode::Existing, context)?;
            Directory::open(&path, false)?.resolved()
        })
    }
}
fn receipt_name(destination: &Path) -> io::Result<String> {
    let path = destination.to_str().ok_or_else(|| invalid("Import destination is not Unicode"))?;
    Ok(format!("{:x}.json", Sha256::digest(path.as_bytes())))
}
fn read_receipt(
    records: &Directory,
    name: &str,
    context: &JobContext,
) -> io::Result<Option<(Receipt, File)>> {
    let mut file = match records.read(name) {
        Ok(file) => file,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(cause) => return Err(cause),
    };
    files::single_link(&file)?;
    protection::with(context, |owner| owner.verify_file(&file))?;
    let receipt =
        Receipt::decode(&files::read_admitted(&mut file, record::MAX_RECORD_BYTES, context)?)?;
    Ok(Some((receipt, file)))
}
fn read_journal(records: &Directory, receipt: &Receipt, context: &JobContext) -> io::Result<File> {
    let mut file = records.read(&format!("{}.json", receipt.stage))?;
    files::single_link(&file)?;
    protection::with(context, |owner| owner.verify_file(&file))?;
    let journal: Journal = serde_json::from_slice(&files::read_admitted(
        &mut file,
        record::MAX_RECORD_BYTES,
        context,
    )?)
    .map_err(io::Error::other)?;
    if journal != receipt.journal {
        return Err(invalid("Import journal differs from its completed receipt"));
    }
    Ok(file)
}
fn open_optional(path: &Path) -> io::Result<Option<Directory>> {
    match Directory::open(path, false) {
        Ok(directory) => Ok(Some(directory)),
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(cause) => Err(cause),
    }
}
