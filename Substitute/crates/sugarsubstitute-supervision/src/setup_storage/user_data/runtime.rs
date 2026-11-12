// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve selected historical namespaces without activating native-bound records.
use super::{host::existing, installation::InstallationData};
use crate::{
    durable_storage::{check, files::Directory, imports, invalid, protection, user_paths},
    native_claims::historical::{self, HistoricalClaims},
};
use std::{
    io,
    path::{Path, PathBuf},
};
use sugarsubstitute_contracts::{execution::JobContext, lifecycle::NamespaceId};

/// Retain original closure evidence only when a pending import needs old process exclusion.
pub(super) struct LegacySources {
    claims: Option<Directory>,
    namespaces: Vec<NamespaceId>,
    closed: Option<HistoricalClaims>,
}
impl LegacySources {
    /// Existing-only native exclusions refuse a live old coordinator/backend or independent Job.
    pub(super) fn require_closed(&mut self, context: &JobContext) -> io::Result<()> {
        if self.closed.is_none()
            && let Some(source) = &self.claims
        {
            self.closed =
                Some(historical::released(source.retained()?, &self.namespaces, context)?);
        }
        check(context)
    }
}

/// Discover the old account location through the actual primary token, without creating it.
/// Completed destinations are checked before any old claim is acquired or original is inspected.
pub(super) fn settle(
    installation: &InstallationData,
    context: &JobContext,
) -> io::Result<LegacySources> {
    let namespaces = installation.with_namespaces(context, |namespaces| Ok(namespaces.to_vec()))?;
    settle_sources(
        &installation.root,
        imports::ImportAssociation::Installed(installation.snapshot.installation()),
        namespaces,
        &account_runtime(context)?,
        context,
    )
}

/// The primary account's earlier runtime folder, read natively and never created.
fn account_runtime(context: &JobContext) -> io::Result<PathBuf> {
    protection::with(context, |owner| owner.historical_runtime(context))
}

/// Source-host namespaces come only from the current elected native directory/account owner.
pub(super) fn settle_source(
    root: &Directory,
    association: imports::ImportAssociation<'_>,
    namespace: NamespaceId,
    context: &JobContext,
) -> io::Result<LegacySources> {
    settle_sources(root, association, vec![namespace], &account_runtime(context)?, context)
}

/// Preserve and import every earlier source below `historical`, the account's earlier
/// `runtime` folder. Callers resolve it natively; qualification supplies an owned stand-in.
pub(super) fn settle_sources(
    root: &Directory,
    association: imports::ImportAssociation<'_>,
    namespaces: Vec<NamespaceId>,
    historical: &Path,
    context: &JobContext,
) -> io::Result<LegacySources> {
    let claims = existing(&historical.join("claims"))?;
    let mut sources = LegacySources { claims, namespaces, closed: None };
    preserve_incident_namespaces(root, association, &mut sources, context)?;
    // Account-wide pages have no installation identity. Their separate provenance-labelled
    // destination grants no ownership over other account history or process namespaces.
    let import = crate::diagnostic_storage::import_original_operational_history;
    import(root, association, historical, "account-history", context)?;
    import(root, association, &root.path().join("runtime"), "installation-history", context)?;
    Ok(sources)
}

fn preserve_incident_namespaces(
    root: &Directory,
    association: imports::ImportAssociation<'_>,
    sources: &mut LegacySources,
    context: &JobContext,
) -> io::Result<()> {
    let target_parent = root.path().join("user/migration-originals/diagnostic-namespaces");
    for namespace in sources.namespaces.clone() {
        let name = crate::durable_storage::protected::record_file::hex(namespace.bytes());
        let target = target_parent.join(&name);
        user_paths::create_parents(root, &target_parent, context)?;
        if completed_for(root, association, &target, context)? {
            continue;
        }
        let Some(claims) = &sources.claims else {
            continue;
        };
        let Some(source) = existing(&claims.path().join("diagnostics/incident-v1").join(&name))?
        else {
            continue;
        };
        sources.require_closed(context)?;
        protection::with(context, |owner| {
            owner.directory(
                source.path(),
                crate::durable_storage::protected::directories::Mode::Existing,
                context,
            )?;
            Ok(())
        })?;
        // Native-bound journals and archives keep their exact original bytes here. This passive
        // copy is never an active store: the crash owner qualifies committed history separately.
        imports::copy_tree(
            root,
            association,
            &source,
            &target,
            imports::RootEntries::Except(&[]),
            context,
        )?;
    }
    Ok(())
}

/// Observe a published native-bound destination independently of later source edits or absence.
pub(super) fn completed(
    installation: &InstallationData,
    destination: &Path,
    context: &JobContext,
) -> io::Result<bool> {
    completed_for(
        &installation.root,
        imports::ImportAssociation::Installed(installation.snapshot.installation()),
        destination,
        context,
    )
}
fn completed_for(
    root: &Directory,
    association: imports::ImportAssociation<'_>,
    destination: &Path,
    context: &JobContext,
) -> io::Result<bool> {
    match imports::verify_completed(root, association, destination, context) {
        Ok(()) => Ok(true),
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(cause) => Err(cause),
    }
}

/// Form an inert bounded Unicode option value without changing the original persisted document.
pub(super) fn option_path(path: PathBuf) -> io::Result<String> {
    path.into_os_string()
        .into_string()
        .map_err(|_| invalid("Settled backend directory is not Unicode"))
}
