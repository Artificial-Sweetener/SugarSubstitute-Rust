// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener

//! Creation is outstanding only after all synchronous preparation succeeds.

use super::Result;

pub(super) fn optional<T>(result: windows::core::Result<T>) -> Result<Option<T>> {
  match result {
    Ok(value) => Ok(Some(value)),
    Err(cause) if cause.code() == windows::Win32::Foundation::E_NOINTERFACE => Ok(None),
    Err(cause) => Err(cause.into()),
  }
}

pub(super) fn submit<T>(
  prepare: impl FnOnce() -> Result<T>,
  arm: impl FnOnce() -> Result<()>,
  dispatch: impl FnOnce(T) -> Result<()>,
) -> Result<()> {
  let prepared = prepare()?;
  arm()?;
  dispatch(prepared)
}
