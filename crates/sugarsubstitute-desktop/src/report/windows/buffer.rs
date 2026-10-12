// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Charge bounded UTF16 conversions before native input copies.
use super::super::{NativeReportView, ReportSurfaceError};
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};
pub(super) struct Text {
    pub(super) units: Vec<u16>,
    _claim: MemoryClaim,
}
impl Text {
    pub(super) fn verify(
        &self,
        hwnd: windows::Win32::Foundation::HWND,
        lease: &MemoryLease,
    ) -> Result<(), ReportSurfaceError> {
        let claim = lease
            .claim((self.units.len() * 2) as u64)
            .map_err(|cause| ReportSurfaceError::new("report_native_readback_credit", cause))?;
        let mut observed = Vec::new();
        observed
            .try_reserve_exact(self.units.len())
            .map_err(|cause| ReportSurfaceError::new("report_native_readback_allocate", cause))?;
        if observed.capacity() > self.units.len() {
            return Err(ReportSurfaceError::rejected("report_native_readback_capacity"));
        }
        observed.resize(self.units.len(), 0);
        // SAFETY: Exact retained control is read into this bounded writable terminated buffer.
        let copied =
            unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowTextW(hwnd, &mut observed) };
        let matches =
            copied >= 0 && copied as usize == self.units.len() - 1 && observed == self.units;
        drop(observed);
        drop(claim);
        if !matches {
            return Err(ReportSurfaceError::rejected("report_native_text_readback"));
        }
        Ok(())
    }
    pub(super) fn new(text: &str, lease: &MemoryLease) -> Result<Self, ReportSurfaceError> {
        let bound = NativeReportView::MAX_BYTES * 2 + 1;
        let count = text.encode_utf16().count() + text.bytes().filter(|b| *b == b'\n').count() + 1;
        if text.contains('\0') || count > bound {
            return Err(ReportSurfaceError::rejected("report_utf16_bound"));
        }
        let claim = lease
            .claim((count * 2 + size_of::<Self>()) as u64)
            .map_err(|cause| ReportSurfaceError::new("report_utf16_credit", cause))?;
        let mut units = Vec::new();
        units
            .try_reserve_exact(count)
            .map_err(|cause| ReportSurfaceError::new("report_utf16_allocate", cause))?;
        if units.capacity() > count {
            return Err(ReportSurfaceError::rejected("report_utf16_capacity"));
        }
        for unit in text.encode_utf16() {
            if unit == 10 {
                units.push(13);
            }
            units.push(unit);
        }
        units.push(0);
        Ok(Self { units, _claim: claim })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use sugarsubstitute_contracts::execution::MemoryRelease;
    struct Release;
    impl MemoryRelease for Release {
        fn release(&self, _: u64) {}
    }
    #[test]
    fn report_native_buffer_retains_unicode_crlf_and_one_final_terminator()
    -> Result<(), Box<dyn Error>> {
        let lease = MemoryLease::new(4096, Box::new(Release));
        let text = Text::new("中文😀\nline\n", &lease)?;
        let expected: Vec<_> = "中文😀\r\nline\r\n\0".encode_utf16().collect();
        assert_eq!(text.units, expected);
        assert!(text.units.capacity() <= expected.len());
        Ok(())
    }
    #[test]
    fn report_native_buffer_preflights_nul_size_and_credit() -> Result<(), Box<dyn Error>> {
        let lease = MemoryLease::new(4096, Box::new(Release));
        assert!(Text::new("bad\0text", &lease).is_err());
        let short = MemoryLease::new(1, Box::new(Release));
        assert!(Text::new("line", &short).is_err());
        let oversized = "x".repeat(super::super::super::NativeReportView::MAX_BYTES * 2 + 1);
        assert!(Text::new(&oversized, &lease).is_err());
        let full = lease.claim(lease.bytes())?;
        drop(full);
        Ok(())
    }
}
