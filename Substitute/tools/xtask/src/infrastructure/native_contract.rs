// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind external native protocol ports to their preserved source and authoritative adapter.
use super::{Boundary, Record};
use crate::{
    error::{Result, require},
    policy::Owner,
};
use std::path::Path;

/// Protocol classification requires the exact retained contract; other boundaries cannot claim it.
pub(super) fn check(root: &Path, record: &Record, owner: &Owner, path: &Path) -> Result<()> {
    if !matches!(record.boundary, Some(Boundary::NativeProtocol)) {
        return require(
            record.external_contract.is_none(),
            "storage: only a native protocol boundary declares an external contract",
        );
    }
    require(
        owner.layer == "adapter"
            && record.path.starts_with("native/")
            && owner.auxiliary_roots.iter().any(|relative| path.starts_with(root.join(relative))),
        "storage: native protocol boundary requires its registered native adapter root",
    )?;
    let persistence: crate::catalog::Persistence =
        crate::policy::load(root, "governance/persistence/catalog.toml")?;
    require(
        record.external_contract.as_ref().is_some_and(|contract| {
            persistence.preserves_source(contract, &record.owner, &record.path)
        }),
        "storage: native boundary must belong to its preserved reviewed protocol",
    )
}
