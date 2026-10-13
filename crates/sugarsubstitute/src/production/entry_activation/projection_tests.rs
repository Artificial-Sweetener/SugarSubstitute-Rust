// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check exact Windows units and authoritative lexical validation in the funded projection.
use super::*;
use std::os::windows::ffi::OsStrExt;
fn bytes(units: &[u16]) -> Vec<u8> {
    units.iter().flat_map(|unit| unit.to_le_bytes()).collect()
}
#[test]
fn projection_preserves_unpaired_profile_path_units() -> Result<(), Box<dyn Error + Send + Sync>> {
    let original = [69, 58, 92, 0xd800, 97, 0xdc01];
    let raw = bytes(&original);
    let projected = path(EntryWindowsPathView::from_le_bytes(&raw)?)?;
    assert_eq!(projected.as_os_str().encode_wide().collect::<Vec<_>>(), original);
    Ok(())
}
#[test]
fn projection_rejects_relative_and_parent_traversal_without_rewriting()
-> Result<(), Box<dyn Error + Send + Sync>> {
    for value in ["profile", r"E:\parent\..\profile"] {
        let raw = bytes(&value.encode_utf16().collect::<Vec<_>>());
        assert!(path(EntryWindowsPathView::from_le_bytes(&raw)?).is_err());
    }
    Ok(())
}
#[test]
fn projection_path_exact_unit_bound_survives_owned_conversion()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let mut units =
        vec![u16::from(b'x'); sugarsubstitute_contracts::coordinator_entry::MAX_ENTRY_PATH_UNITS];
    units[..3].copy_from_slice(&[69, 58, 92]);
    let raw = bytes(&units);
    let projected = path(EntryWindowsPathView::from_le_bytes(&raw)?)?;
    assert_eq!(projected.as_os_str().encode_wide().collect::<Vec<_>>(), units);
    Ok(())
}
