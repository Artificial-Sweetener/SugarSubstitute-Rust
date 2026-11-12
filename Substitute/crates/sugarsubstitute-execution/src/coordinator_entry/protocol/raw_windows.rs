// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Borrow native Windows path units without normalization or temporary allocations.
use super::EntryTransportError;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

#[cfg(windows)]
pub(super) fn units(path: &Path) -> Result<impl Iterator<Item = u16> + '_, EntryTransportError> {
    Ok(path.as_os_str().encode_wide())
}
#[cfg(not(windows))]
pub(super) fn units(path: &Path) -> Result<impl Iterator<Item = u16> + '_, EntryTransportError> {
    Ok(path.to_str().ok_or(EntryTransportError::Malformed)?.encode_utf16())
}
pub(super) fn length(path: &Path) -> Result<usize, EntryTransportError> {
    units(path)?.count().checked_mul(2).ok_or(EntryTransportError::Capacity)
}
