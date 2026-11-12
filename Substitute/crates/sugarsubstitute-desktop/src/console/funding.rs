// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit at most one copied document from each exact original frame allocation.
use crate::DesktopError;
use sugarsubstitute_contracts::{
    execution::MemoryClaim,
    presentation::{NATIVE_CONSOLE_STORAGE_BYTES, PreparedFrame},
};
/// A strong original frame keeps physical allocation identity stable across exposures.
/// Cloning shares existing pixels/credit; this owner allocates or scans no pixel data.
pub(crate) struct Funding {
    frame: PreparedFrame,
}
impl Funding {
    /// Claim before allocating the first native document or callback state.
    pub(crate) fn new(frame: &PreparedFrame) -> Result<(Self, MemoryClaim), DesktopError> {
        let claim = frame
            .claim_storage(NATIVE_CONSOLE_STORAGE_BYTES)
            .map_err(|e| DesktopError::new("console_claim", e))?;
        Ok((Self { frame: frame.clone() }, claim))
    }
    /// Repeat exposure coalesces newer text until another genuine frame can fund it.
    /// Equal metadata does not substitute for the same held pixel allocation. A rejected
    /// new claim preserves the original allocation and admits no copied storage.
    pub(crate) fn next(
        &mut self,
        frame: &PreparedFrame,
    ) -> Result<Option<MemoryClaim>, DesktopError> {
        if std::ptr::eq(self.frame.pixels(), frame.pixels()) {
            return Ok(None);
        }
        let claim = frame
            .claim_storage(NATIVE_CONSOLE_STORAGE_BYTES)
            .map_err(|e| DesktopError::new("console_claim", e))?;
        self.frame = frame.clone();
        Ok(Some(claim))
    }
}
#[cfg(test)]
mod tests;
