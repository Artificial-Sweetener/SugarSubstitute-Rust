// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare an explicit Unicode child environment without ambient process mutation.
//!
//! Windows requires locale-independent case-insensitive name ordering and a final
//! double NUL. This owner preserves exact UTF-16 units, including lone surrogates.
//! Native launch must supply `CREATE_UNICODE_ENVIRONMENT`; construction grants no
//! process privilege, filesystem trust or bootstrap capability.
//! See [Windows environments](https://learn.microsoft.com/en-us/windows/win32/procthread/changing-environment-variables).
mod backend;
mod entries;
mod errors;
#[allow(unsafe_code)]
mod ordinal;
mod storage;
pub use errors::LaunchEnvironmentError;
use std::fmt;

const MAX_UNITS: usize = 32767;
const MAX_VARIABLES: usize = 1024;
const MAX_OVERRIDES: usize = 64;

/// One output block holds at most the enforced unit limit and is reserved exactly.
const BLOCK_BYTES: usize = MAX_UNITS * size_of::<u16>();
/// The merge table reserves one slot per snapshot variable and per override.
const MERGE_TABLE_BYTES: usize =
    (MAX_VARIABLES + MAX_OVERRIDES) * size_of::<entries::Entry<'static>>();
/// The sorted override table `merge` builds beside the merge table.
const OVERRIDE_TABLE_BYTES: usize = MAX_OVERRIDES * size_of::<entries::Entry<'static>>();
/// Override descriptors plus the two owned vectors and name text each override can carry.
const CONSTRUCTOR_SCRATCH_BYTES: usize =
    MAX_OVERRIDES * (size_of::<EnvironmentOverride<'static>>() + 2 * size_of::<Vec<u16>>() + 80);

const fn larger(left: usize, right: usize) -> usize {
    if left > right { left } else { right }
}
/// Override scratch, both merge tables and the output block exist together at the widest moment.
const fn local_storage_peak() -> usize {
    CONSTRUCTOR_SCRATCH_BYTES + MERGE_TABLE_BYTES + OVERRIDE_TABLE_BYTES + BLOCK_BYTES
}

/// Borrow an explicit ordinary-variable change for this child only.
/// Names exclude NUL and `=`; values exclude NUL and may be empty. Hidden shell
/// entries are snapshot-only. Project-private variables cannot be inserted.
pub enum EnvironmentOverride<'a> {
    /// Replace or insert one case-insensitively matched ordinary variable.
    Set {
        /// Exact UTF-16 name; no ambient lookup is performed.
        name: &'a [u16],
        /// Exact UTF-16 value, including literal shell characters.
        value: &'a [u16],
    },
    /// Remove one ordinary variable; an absent name is a successful no-op.
    Remove {
        /// Exact UTF-16 matching name.
        name: &'a [u16],
    },
}
impl fmt::Debug for EnvironmentOverride<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EnvironmentOverride").finish_non_exhaustive()
    }
}

/// Own one validated, sorted, double-NUL-terminated Unicode environment block.
/// It is neither cloned nor attached to ambient environment/DLL-search state.
pub struct LaunchEnvironment {
    block: Vec<u16>,
}
impl fmt::Debug for LaunchEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LaunchEnvironment").finish_non_exhaustive()
    }
}
impl LaunchEnvironment {
    /// Prepare a child from an explicitly supplied exact snapshot and changes.
    ///
    /// Bounds are checked before allocation/content scans: snapshot and aggregate
    /// override input each have at most 32767 UTF-16 units, with at most 64 changes.
    /// Snapshot/final variable counts are at most 1024. Output, including double
    /// NUL, is at most 32767 units. Duplicate names reject under Windows ordinal
    /// ignore-case comparison. Snapshot-only hidden shell entries are retained.
    /// Snapshot project-private prefixes are stripped; insertion is rejected.
    /// No environment capture, process launch or capability injection occurs.
    ///
    /// # Errors
    /// Rejects malformed termination/names, NUL values, duplicates and declared
    /// bounds. Original allocation/ordinal API causes remain owned and formatting
    /// redacts input. Failure returns no partially prepared environment.
    pub fn from_snapshot_utf16(
        snapshot: &[u16],
        overrides: &[EnvironmentOverride<'_>],
    ) -> Result<Self, LaunchEnvironmentError> {
        entries::preflight(snapshot, overrides)?;
        let entries = entries::merge(snapshot, overrides)?;
        Self::from_entries(entries)
    }
    /// Serialize entries already validated by this owner's closed constructors.
    fn from_entries(entries: Vec<entries::Entry<'_>>) -> Result<Self, LaunchEnvironmentError> {
        let mut length = 1_usize;
        for entry in &entries {
            let value = entry.value.ok_or(LaunchEnvironmentError::InvalidName)?;
            length = length
                .checked_add(entry.name.len() + value.len() + 2)
                .filter(|length| *length <= MAX_UNITS)
                .ok_or(LaunchEnvironmentError::BlockTooLong)?;
        }
        length = length.max(2);
        let mut block = Vec::new();
        block.try_reserve_exact(length).map_err(LaunchEnvironmentError::Allocation)?;
        for entry in entries {
            let value = entry.value.ok_or(LaunchEnvironmentError::InvalidName)?;
            block.extend_from_slice(entry.name);
            block.push(b'=' as u16);
            block.extend_from_slice(value);
            block.push(0);
        }
        block.push(0);
        if block.len() == 1 {
            block.push(0);
        }
        Ok(Self { block })
    }
    /// Copy this validated owned block exactly, preserving its typed backend semantics.
    /// This performs no ambient capture, override insertion or process launch.
    /// # Errors
    /// Preserves allocation failures; the original environment remains unchanged.
    pub fn try_copy(&self) -> Result<Self, LaunchEnvironmentError> {
        let mut block = Vec::new();
        block.try_reserve_exact(self.block.len()).map_err(LaunchEnvironmentError::Allocation)?;
        block.extend_from_slice(&self.block);
        Ok(Self { block })
    }
    /// Borrow the exact immutable native block, including its terminating double NUL.
    pub fn as_utf16(&self) -> &[u16] {
        &self.block
    }
    /// Transfer the native buffer; subsequent caller mutation invalidates guarantees.
    pub fn into_utf16(self) -> Vec<u16> {
        self.block
    }
    /// Bound new output and borrowed-entry storage for a copy with no overrides.
    /// This inspects this owner's validated original block without allocation,
    /// environment discovery or native launch. The caller keeps its input charge.
    /// # Errors
    /// Retains original snapshot/overflow rejection before any copying effect.
    pub fn copy_storage_bytes(&self) -> Result<u64, LaunchEnvironmentError> {
        let entries = entries::snapshot_count(&self.block)?;
        entries
            .checked_mul(size_of::<entries::Entry<'_>>())
            .and_then(|bytes| {
                self.block
                    .len()
                    .checked_mul(size_of::<u16>())
                    .and_then(|output| bytes.checked_add(output))
            })
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(LaunchEnvironmentError::BlockTooLong)
    }
    /// Bound the same no-overrides copy before a concrete validated block exists.
    /// The values are this owner's unchanged enforced unit/variable limits.
    pub const fn maximum_copy_storage_bytes() -> u64 {
        (MAX_VARIABLES * size_of::<entries::Entry<'static>>() + MAX_UNITS * size_of::<u16>()) as u64
    }

    /// Bound the peak buffers one [`Self::for_local_storage`] build owns beside its snapshot.
    ///
    /// Every merge table and output block is reserved exactly and limited by this owner's
    /// enforced unit, variable and override counts, so the peak never exceeds this constant for
    /// any admitted snapshot. Path-sized buffers the caller derives from the storage directory
    /// are not included; the caller charges them from the actual path length.
    pub const fn maximum_local_storage_bytes() -> u64 {
        local_storage_peak() as u64
    }

    /// Bound the peak buffers a local-storage build followed by one [`Self::for_managed_backend`]
    /// build own beside their snapshot, counting the local-storage block that stays retained.
    ///
    /// The backend build reuses no buffer of the local-storage build: it holds the retained
    /// block, its own intermediate block, the PATH scratch (reallocated once), and then the
    /// merged table beside the final block. The widest of those moments is the bound, so it
    /// never exceeds this constant for any admitted snapshot.
    pub const fn maximum_managed_backend_bytes() -> u64 {
        let initial_table = MAX_VARIABLES * size_of::<entries::Entry<'static>>();
        // Intermediate block, initial table, and PATH scratch while it grows beside its old copy.
        let search_path_growth = BLOCK_BYTES + initial_table + 2 * BLOCK_BYTES;
        // Intermediate block, PATH scratch, and the merged table while it grows by one entry.
        let merged_table_growth = 2 * BLOCK_BYTES + 2 * MERGE_TABLE_BYTES;
        // Intermediate block, PATH scratch, merged table, and the final block.
        let final_block = 3 * BLOCK_BYTES + MERGE_TABLE_BYTES;
        let backend = larger(
            local_storage_peak(),
            larger(search_path_growth, larger(merged_table_growth, final_block)),
        );
        (BLOCK_BYTES + CONSTRUCTOR_SCRATCH_BYTES + backend) as u64
    }

    /// Account actual retained buffer capacity in bytes, excluding this stack owner.
    /// Opaque native allocations and borrowed preparation metadata are not included.
    /// This observable query grants no memory reservation or admission authority.
    ///
    /// # Errors
    /// Returns `BlockTooLong` if the actual capacity cannot be represented in bytes.
    pub fn allocated_bytes(&self) -> Result<u64, LaunchEnvironmentError> {
        self.block
            .capacity()
            .checked_mul(size_of::<u16>())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(LaunchEnvironmentError::BlockTooLong)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accounting_measures_actual_retained_capacity() -> Result<(), LaunchEnvironmentError> {
        let environment = LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?;
        assert_eq!(environment.allocated_bytes()?, (environment.block.capacity() * 2) as u64);
        assert!(environment.allocated_bytes()? >= 4);
        Ok(())
    }
    #[test]
    fn no_override_copy_bound_covers_actual_output_and_private_entry_layouts()
    -> Result<(), LaunchEnvironmentError> {
        for raw in [vec![0, 0], "A=one\0B=two\0\0".encode_utf16().collect()] {
            let original = LaunchEnvironment::from_snapshot_utf16(&raw, &[])?;
            let declared = original.copy_storage_bytes()?;
            let copied = LaunchEnvironment::from_snapshot_utf16(original.as_utf16(), &[])?;
            assert!(declared >= copied.allocated_bytes()?);
            assert!(declared <= LaunchEnvironment::maximum_copy_storage_bytes());
            assert_eq!(copied.as_utf16(), original.as_utf16());
        }
        Ok(())
    }
}
