// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Allocate only copied units under the caller's already retained preparation charge.

use super::cause::EntryEnvironmentCause;
use sugarsubstitute_contracts::coordinator_entry::MAX_ENTRY_ENVIRONMENT_UNITS;

pub(super) fn copy_storage(length: usize) -> Result<Vec<u16>, EntryEnvironmentCause> {
    if length > MAX_ENTRY_ENVIRONMENT_UNITS {
        return Err(EntryEnvironmentCause::TooLarge { inspected_units: length });
    }
    let mut units = Vec::new();
    units
        .try_reserve_exact(length)
        .map_err(|source| EntryEnvironmentCause::CopyAllocation { source })?;
    if units.capacity() > MAX_ENTRY_ENVIRONMENT_UNITS {
        return Err(EntryEnvironmentCause::CopyCapacity {
            actual_capacity_units: units.capacity(),
        });
    }
    units.resize(length, 0);
    Ok(units)
}
