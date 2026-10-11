// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify only pure fixed-layout decisions; no native handles/APIs are created.
use super::*;
#[test]
fn allowlist_has_exact_zero_one_three_and_four_handle_layouts() -> Result<(), RegistryError> {
    assert_eq!(allowlist(None, None, None)?, ([0; 4], 0));
    assert_eq!(allowlist(None, Some(41), None)?, ([41, 0, 0, 0], 1));
    assert_eq!(allowlist(None, None, Some([71, 72, 73]))?, ([71, 72, 73, 0], 3));
    assert_eq!(allowlist(None, Some(41), Some([71, 72, 73]))?, ([41, 71, 72, 73], 4));
    assert_eq!(allowlist(Some([31, 32, 33, 34]), None, None)?, ([31, 32, 33, 34], 4));
    Ok(())
}
#[test]
fn allowlist_rejects_mutually_exclusive_roles_invalid_and_duplicate_handles() {
    assert!(allowlist(Some([31, 32, 33, 34]), Some(41), None).is_err());
    assert!(allowlist(Some([31, 32, 33, 34]), None, Some([71, 72, 73])).is_err());
    assert!(allowlist(None, Some(71), Some([71, 72, 73])).is_err());
    assert!(allowlist(None, None, Some([71, 71, 73])).is_err());
    assert!(allowlist(None, None, Some([0, 72, 73])).is_err());
    assert!(allowlist(None, None, Some([usize::MAX, 72, 73])).is_err());
}
