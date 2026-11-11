// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain clipboard memory/close obligations across actual native boundary failures.
mod native;
#[cfg(test)]
mod tests;
use super::super::ReportSurfaceError;
use native::Native;
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};
use windows::Win32::Foundation::{HGLOBAL, HWND};
/// Private real native boundary: successful write owns one lock; transfer hands ownership to OS.
pub(super) trait Boundary {
    fn allocate(&mut self, bytes: usize) -> Result<HGLOBAL, ReportSurfaceError>;
    fn write(&mut self, global: HGLOBAL, units: &[u16]) -> Result<(), ReportSurfaceError>;
    fn unlock(&mut self, global: HGLOBAL) -> Result<(), ReportSurfaceError>;
    fn open(&mut self) -> Result<(), ReportSurfaceError>;
    fn empty(&mut self) -> Result<(), ReportSurfaceError>;
    fn transfer(&mut self, global: HGLOBAL) -> Result<HGLOBAL, ReportSurfaceError>;
    fn close(&mut self) -> Result<(), ReportSurfaceError>;
    fn free(&mut self, global: HGLOBAL) -> Result<(), ReportSurfaceError>;
}
pub(super) struct Clipboard<B = Native> {
    boundary: B,
    global: Option<HGLOBAL>,
    open: bool,
    locked: bool,
    claim: Option<MemoryClaim>,
}
impl Clipboard<Native> {
    pub(super) const fn new(parent: HWND) -> Self {
        Self { boundary: Native { parent }, global: None, open: false, locked: false, claim: None }
    }
}
impl<B: Boundary> Clipboard<B> {
    pub(super) fn copy(
        &mut self,
        units: &[u16],
        lease: &MemoryLease,
    ) -> Result<(), ReportSurfaceError> {
        if self.global.is_some()
            || self.open
            || units.last() != Some(&0)
            || units[..units.len().saturating_sub(1)].contains(&0)
            || units.len() > super::super::NativeReportView::MAX_BYTES * 2 + 1
        {
            return Err(ReportSurfaceError::rejected("report_clipboard_admission"));
        }
        let claim = lease
            .claim((units.len() * 2) as u64)
            .map_err(|cause| ReportSurfaceError::new("report_clipboard_credit", cause))?;
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
            return Err(ReportSurfaceError::rejected("report_clipboard_transfer_identity"));
        }
        self.close()
    }
    fn close(&mut self) -> Result<(), ReportSurfaceError> {
        if self.open {
            self.boundary.close()?;
            self.open = false;
        }
        Ok(())
    }
    pub(super) fn retire(&mut self) -> Result<(), ReportSurfaceError> {
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
