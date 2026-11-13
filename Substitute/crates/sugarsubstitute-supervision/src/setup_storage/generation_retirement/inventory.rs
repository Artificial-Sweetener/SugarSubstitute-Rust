// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Describe what a generation folder's owners wrote into it, from the folder's own records.
//!
//! A generation holds its package's `app/` files at the folder root, the retained package
//! inventory, installation-owned records beside them and, after an interruption, stage files.
//! The package inventory and the generation marker are deleted last, so a resumed retirement
//! reads the same inventory again. Without a readable package inventory only the
//! installation-owned records are known; the rest of the folder is preserved.
use super::super::{check, inventory::INSTALLED_MANIFEST, transaction::generation::INITIAL_PREFIX};
use crate::durable_storage::{
    files::{self, Directory},
    owned_retirement::{RetirementError, Tombstone, TreeInventory},
};
use std::io;
use sugarsubstitute_contracts::{execution::JobContext, installation::GenerationId};
use sugarsubstitute_update::package::{MAX_MANIFEST_BYTES, Manifest};

/// Installation-owned records written beside a generation's package files.
const RECORDS: [&str; 7] = [
    "setup-manifest.json",
    "installation-ready.json",
    "application-update-v1.record",
    "installation-authorization-v1.record",
    "startup.local.toml",
    "sugarsubstitute-launch.local.json",
    "SugarSubstitute.lnk",
];
/// The generation marker, written first when a generation folder is created.
const MARKER: &str = "setup-generation.json";
/// Stage files the generation's writers leave when interrupted before publishing a file.
const STAGES: [&str; 4] =
    [".payload-stage-", ".record-stage-", ".setup-entry-stage-", ".substitute-publication-"];

/// Whether `name` is an interrupted, never-published generation folder creation.
pub(super) fn is_initializing(name: &str) -> bool {
    name.len() > INITIAL_PREFIX.len() && name.starts_with(INITIAL_PREFIX)
}

/// The inventory of the generation folder `name` of `releases`, read from its own package
/// inventory. Every handle it opens is closed before it returns, so retirement can rename it.
/// # Errors
/// Preserves native and cancellation causes. A missing or unreadable package inventory is not
/// an error: the package files then stay preserved.
pub(super) fn of_generation(
    releases: &Directory,
    name: &str,
    context: &JobContext,
) -> Result<TreeInventory, RetirementError> {
    check(context)?;
    let manifest = {
        let generation = match releases.child(name) {
            Ok(generation) => generation,
            // A link or other object in a generation's place is preserved by retirement.
            Err(cause) if cause.kind() == io::ErrorKind::InvalidInput => {
                return Ok(TreeInventory::default());
            }
            Err(cause) => return Err(cause.into()),
        };
        match generation.read(INSTALLED_MANIFEST) {
            Ok(mut file) => Some(files::read_bounded(&mut file, MAX_MANIFEST_BYTES)?),
            Err(cause)
                if matches!(
                    cause.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::InvalidInput
                ) =>
            {
                None
            }
            Err(cause) => return Err(cause.into()),
        }
    };
    describe(name, manifest.as_deref())
}

/// The inventory of a tombstone left by an interrupted retirement, read from the records it
/// still holds. A tombstone of anything other than a generation is described as empty, so its
/// content is preserved.
/// # Errors
/// Preserves native and cancellation causes.
pub(super) fn of_tombstone(
    tombstone: &Tombstone<'_>,
    context: &JobContext,
) -> Result<TreeInventory, RetirementError> {
    if is_initializing(tombstone.leaf()) {
        return initializing();
    }
    if GenerationId::validate(tombstone.leaf()).is_err() {
        return Ok(TreeInventory::default());
    }
    let manifest = tombstone.read(INSTALLED_MANIFEST, MAX_MANIFEST_BYTES, context)?;
    describe(tombstone.leaf(), manifest.as_deref())
}

/// The inventory of an interrupted generation creation: only its marker.
pub(super) fn initializing() -> Result<TreeInventory, RetirementError> {
    TreeInventory::new(std::iter::empty())?.finally([MARKER.to_owned()])
}

/// Combine the package's `app/` files named by `manifest` with the installation's records.
fn describe(name: &str, manifest: Option<&[u8]>) -> Result<TreeInventory, RetirementError> {
    let package = match manifest.map(Manifest::decode) {
        Some(Ok(manifest)) => manifest
            .entries()
            .iter()
            .filter_map(|entry| entry.path().strip_prefix("app/").map(str::to_owned))
            .collect(),
        Some(Err(_)) => {
            tracing::warn!(
                operation = "generation_retirement_inventory_unreadable",
                generation = name,
                "Old version cleanup kept the files of a version whose inventory is damaged"
            );
            Vec::new()
        }
        None => Vec::new(),
    };
    let mut inventory = TreeInventory::new(
        package.into_iter().chain(RECORDS.iter().map(|record| (*record).to_owned())),
    )?
    .finally([INSTALLED_MANIFEST.to_owned(), MARKER.to_owned()])?;
    for prefix in STAGES {
        inventory = inventory.stages(prefix)?;
    }
    Ok(inventory)
}
