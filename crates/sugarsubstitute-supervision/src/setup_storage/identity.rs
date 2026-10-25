// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Allocate installation and generation identities without granting accepted-release authority.
use crate::durable_storage::staging;
use std::io;
use sugarsubstitute_contracts::installation::{GenerationId, InstallationId};

/// Allocate the canonical directory identity used by durable installation selection.
pub(super) fn generation() -> io::Result<GenerationId> {
    GenerationId::new(staging::identity()?).map_err(io::Error::other)
}

/// Allocate a persistent installation identity without claiming an accepted generation.
pub(super) fn installation() -> io::Result<InstallationId> {
    InstallationId::new(staging::identity()?).map_err(io::Error::other)
}
