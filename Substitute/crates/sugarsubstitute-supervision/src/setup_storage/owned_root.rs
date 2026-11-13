// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit a folder of an owned installation for ownership-verified retirement.
use super::ownership;
use crate::durable_storage::{
    files::Directory,
    owned_retirement::{OwnedRoot, RetirementError},
};
use std::path::Path;
use sugarsubstitute_contracts::execution::JobContext;

/// Pin `root`, a folder below the installation at `installation`, for retirement.
///
/// The installation's ownership marker is verified and held for the returned root's lifetime.
/// `root` must be a strict descendant of the installation reached without links or junctions,
/// and must lie outside the installation's own state folder. Runs on an admitted worker.
/// # Errors
/// Returns [`RetirementError::Ownership`] when the marker cannot be verified or `root` is not
/// confined below the installation, and `Storage` when `root` cannot be opened.
pub fn owned_retirement_root(
    installation: &Path,
    root: &Path,
    context: &JobContext,
) -> Result<OwnedRoot, RetirementError> {
    super::check(context)?;
    let installation = Directory::open(installation, false).map_err(RetirementError::Ownership)?;
    let marker = ownership::read(&installation).map_err(RetirementError::Ownership)?;
    let state = ownership::state(&installation).map_err(RetirementError::Ownership)?;
    let directory = Directory::open(root, false)?;
    let confined = directory.identity()? != installation.identity()?
        && directory.has_ancestor(&installation, context)?
        && !directory.has_ancestor(&state, context)?;
    if !confined {
        return Err(RetirementError::Ownership(super::invalid(
            "Retirement folder is not confined below its installation",
        )));
    }
    Ok(OwnedRoot::verified(directory, marker))
}
