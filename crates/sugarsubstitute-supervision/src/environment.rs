// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare an explicit Unicode child environment without ambient process mutation.
//!
//! Windows requires locale-independent case-insensitive name ordering and a final
//! double NUL. This owner preserves exact UTF-16 units, including lone surrogates.
//! Native launch must supply `CREATE_UNICODE_ENVIRONMENT`; construction grants no
//! process privilege, filesystem trust or bootstrap capability.
//! See [Windows environments](https://learn.microsoft.com/en-us/windows/win32/procthread/changing-environment-variables).
mod entries;
mod errors;
#[allow(unsafe_code)]
mod ordinal;
pub use errors::LaunchEnvironmentError;
use std::fmt;

const MAX_UNITS: usize = 32767;
const MAX_VARIABLES: usize = 1024;
const MAX_OVERRIDES: usize = 64;

/// Borrow an explicit ordinary-variable change for this child only.
/// Names exclude NUL and `=`; values exclude NUL and may be empty. Hidden drive
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
    /// ignore-case comparison. Snapshot-only `=X:` drive entries are retained.
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
