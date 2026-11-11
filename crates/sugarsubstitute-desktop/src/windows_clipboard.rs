// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer Unicode text to the clipboard, retaining memory and close obligations across
//! native boundary failures until they are discharged.
pub(crate) mod native;
#[cfg(test)]
mod tests;
use crate::DesktopError;
use native::Native;

/// Largest transfer in UTF-16 units, including the terminator: a 256 KiB report.
pub(crate) const MAX_UNITS: usize = 512 * 1024 + 1;
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};
use windows::Win32::Foundation::{HGLOBAL, HWND};
/// Private real native boundary: successful write owns one lock; transfer hands ownership to OS.
pub(crate) trait Boundary {
    fn allocate(&mut self, bytes: usize) -> Result<HGLOBAL, DesktopError>;
    fn write(&mut self, global: HGLOBAL, units: &[u16]) -> Result<(), DesktopError>;
    fn unlock(&mut self, global: HGLOBAL) -> Result<(), DesktopError>;
    fn open(&mut self) -> Result<(), DesktopError>;
    fn empty(&mut self) -> Result<(), DesktopError>;
    fn transfer(&mut self, global: HGLOBAL) -> Result<HGLOBAL, DesktopError>;
    fn close(&mut self) -> Result<(), DesktopError>;
    fn free(&mut self, global: HGLOBAL) -> Result<(), DesktopError>;
}
pub(crate) struct Clipboard<B = Native> {
    boundary: B,
    global: Option<HGLOBAL>,
    open: bool,
    locked: bool,
    claim: Option<MemoryClaim>,
}
impl Clipboard<Native> {
    pub(crate) const fn new(parent: HWND) -> Self {
        Self { boundary: Native { parent }, global: None, open: false, locked: false, claim: None }
    }
}
impl<B: Boundary> Clipboard<B> {
    pub(crate) fn copy(&mut self, units: &[u16], lease: &MemoryLease) -> Result<(), DesktopError> {
        if self.global.is_some()
            || self.open
            || units.last() != Some(&0)
            || units[..units.len().saturating_sub(1)].contains(&0)
            || units.len() > MAX_UNITS
        {
            return Err(DesktopError::message(
                "report_clipboard_admission",
                "native clipboard contract violated",
            ));
        }
        let claim = lease
            .claim((units.len() * 2) as u64)
            .map_err(|cause| DesktopError::new("report_clipboard_credit", cause))?;
        let global = self.boundary.allocate(units.len() * 2)?;
        self.global = Some(global);
        self.claim = Some(claim);
        self.boundary.write(global, units)?;
        self.locked = true;
        self.boundary.unlock(global)?;
        self.locked = false;
        self.boundary.open()?;
        self.open = true;
        self.boundary.empty()?;
        let transferred = self.boundary.transfer(global)?;
        // A successful OS transfer discharges local memory even if returned correlation disagrees.
        self.global = None;
        self.claim = None;
        if transferred != global {
            return Err(DesktopError::message(
                "report_clipboard_transfer_identity",
                "native clipboard contract violated",
            ));
        }
        self.close()
    }
    fn close(&mut self) -> Result<(), DesktopError> {
        if self.open {
            self.boundary.close()?;
            self.open = false;
        }
        Ok(())
    }
    pub(crate) fn retire(&mut self) -> Result<(), DesktopError> {
        self.close()?;
        if let Some(global) = self.global {
            if self.locked {
                self.boundary.unlock(global)?;
                self.locked = false;
            }
            self.boundary.free(global)?;
            self.global = None;
            self.claim = None;
        }
        Ok(())
    }
}

/// Copy `text` to the clipboard as the owner `window`, discharging every native obligation
/// before returning. Text containing NUL is rejected.
/// # Errors
/// Preserves the first native failure; cleanup failures are logged after it.
pub(crate) fn copy_text(window: HWND, text: &str) -> Result<(), DesktopError> {
    let mut units: Vec<u16> = text.encode_utf16().collect();
    units.push(0);
    let lease = MemoryLease::new((units.len() * 2) as u64, Box::new(Standalone));
    let mut clipboard = Clipboard::new(window);
    let copied = clipboard.copy(&units, &lease);
    let retired = clipboard.retire();
    match (copied, retired) {
        (Err(cause), Err(cleanup)) => {
            tracing::error!(operation = "clipboard_retire", cause = %cleanup, "Clipboard cleanup failed after a failed copy");
            Err(cause)
        }
        (Err(cause), Ok(())) | (Ok(()), Err(cause)) => Err(cause),
        (Ok(()), Ok(())) => Ok(()),
    }
}

/// Clipboard text belongs to the window that copies it, not to an execution budget.
struct Standalone;
impl sugarsubstitute_contracts::execution::MemoryRelease for Standalone {
    fn release(&self, _: u64) {}
}
