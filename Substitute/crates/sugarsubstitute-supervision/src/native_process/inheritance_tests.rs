// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify only pure fixed-layout decisions; no native handles/APIs are created.
use super::*;
#[test]
fn allowlist_has_exact_zero_one_three_and_four_handle_layouts() -> Result<(), RegistryError> {
    assert_eq!(allowlist(None, None, None, None)?, ([0; 6], 0));
    assert_eq!(allowlist(None, Some(41), None, None)?, ([41, 0, 0, 0, 0, 0], 1));
    assert_eq!(allowlist(None, None, None, Some([71, 72, 73]))?, ([71, 72, 73, 0, 0, 0], 3));
    assert_eq!(allowlist(None, Some(41), None, Some([71, 72, 73]))?, ([41, 71, 72, 73, 0, 0], 4));
    assert_eq!(allowlist(Some([31, 32, 33, 34]), None, None, None)?, ([31, 32, 33, 34, 0, 0], 4));
    Ok(())
}
#[test]
fn allowlist_rejects_mutually_exclusive_roles_invalid_and_duplicate_handles() {
    assert!(allowlist(Some([31, 32, 33, 34]), Some(41), None, None).is_err());
    assert!(allowlist(Some([31, 32, 33, 34]), None, None, Some([71, 72, 73])).is_err());
    assert!(allowlist(None, Some(71), None, Some([71, 72, 73])).is_err());
    assert!(allowlist(None, None, None, Some([71, 71, 73])).is_err());
    assert!(allowlist(None, None, None, Some([0, 72, 73])).is_err());
    assert!(allowlist(None, None, None, Some([usize::MAX, 72, 73])).is_err());
}

#[test]
fn captured_bootstrap_keeps_both_real_process_slots_and_optional_stdio() -> Result<(), RegistryError>
{
    assert_eq!(allowlist(None, Some(41), Some([51, 52]), None)?, ([41, 51, 52, 0, 0, 0], 3));
    assert_eq!(
        allowlist(None, Some(41), Some([51, 52]), Some([71, 72, 73]))?,
        ([41, 51, 52, 71, 72, 73], 6)
    );
    Ok(())
}
#[test]
fn captured_allowlist_rejects_missing_section_conflicts_and_every_duplicate_or_pseudo_slot() {
    assert!(allowlist(None, None, Some([51, 52]), None).is_err());
    assert!(allowlist(Some([31, 32, 33, 34]), None, Some([51, 52]), None).is_err());
    for captured in [[0, 52], [usize::MAX, 52], [51, 51], [41, 52], [51, 71]] {
        assert!(allowlist(None, Some(41), Some(captured), Some([71, 72, 73])).is_err());
    }
}
#[test]
fn captured_family_layout_remains_inside_existing_admitted_storage() {
    assert!(
        std::mem::size_of::<Family>() + crate::registry::family_shared_storage_bytes()
            < crate::registry_input::FAMILY_STORAGE_BYTES as usize
    );
}
