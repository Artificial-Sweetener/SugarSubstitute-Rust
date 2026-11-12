// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Accept checked charged inert view transfer without granting native authority.
use super::ReportSurfaceError;
use sugarsubstitute_contracts::{
    execution::{MemoryClaim, MemoryLease},
    incident::{MAX_TEXT_ATTACHMENT_BYTES, ReportBinding, ReportRestartAvailability},
    localization::{Messages, messages},
};
/// Complete immutable redacted report text and the same transferred allocation claim.
pub struct NativeReportView {
    text: String,
    binding: ReportBinding,
    restart: ReportRestartAvailability,
    _claim: MemoryClaim,
}
impl NativeReportView {
    /// Maximum retained UTF8 text allocation, including omission marker and final LF.
    pub const MAX_BYTES: usize = MAX_TEXT_ATTACHMENT_BYTES as usize;
    /// Consume checked inert data plus its actual noncloneable allocation credit.
    ///
    /// # Errors
    /// Rejects empty/oversized capacity, embedded NUL/CR, missing final LF or insufficient
    /// claim. This authenticates no peer and grants no clipboard/file/process authority.
    pub fn from_claimed(
        binding: ReportBinding,
        text: String,
        claim: MemoryClaim,
        restart: ReportRestartAvailability,
    ) -> Result<Self, ReportSurfaceError> {
        if text.is_empty()
            || text.len() > Self::MAX_BYTES
            || text.capacity() > Self::MAX_BYTES
            || text.contains(['\0', '\r'])
            || !text.ends_with('\n')
            || claim.bytes() < (text.capacity() + size_of::<Self>()) as u64
        {
            return Err(ReportSurfaceError::rejected("report_view_transfer"));
        }
        Ok(Self { text, binding, restart, _claim: claim })
    }
    /// Borrow exact redacted text; no raw native error formatting is performed.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Return the authenticated composing host's inert presentation decision.
    pub const fn restart(&self) -> ReportRestartAvailability {
        self.restart
    }
    /// Return matching facts; construction did not authenticate the sender.
    pub const fn binding(&self) -> ReportBinding {
        self.binding
    }
}
/// Bound the native surface independently of browser, backend and failed desktop state.
pub struct ReportSurfaceOptions {
    pub(super) view: NativeReportView,
    pub(super) copy: &'static Messages,
    pub(super) lease: MemoryLease,
    pub(super) dispatch_storage: sugarsubstitute_contracts::execution::MemoryClaim,
}
impl ReportSurfaceOptions {
    /// Known authored native buffer/header ceiling, excluding opaque OS widget allocations.
    pub const KNOWN_STORAGE_BYTES: u64 = 4 * 1024 * 1024;
    /// Fixed ceiling for the single native Session/content box and UI-only dispatch metadata.
    pub const DISPATCH_STORAGE_BYTES: u64 = 64 * 1024;
    /// Select canonical localized copy before native effects. The report window follows the
    /// system theme and sizes itself to the report and the screen.
    ///
    /// # Errors
    /// Rejects insufficient native credit, including the fixed dispatch reservation within the
    /// same known storage ceiling. The transferred view claim is retained through teardown.
    pub fn new(
        view: NativeReportView,
        locale: &str,
        lease: MemoryLease,
    ) -> Result<Self, ReportSurfaceError> {
        if lease.bytes() < Self::KNOWN_STORAGE_BYTES {
            return Err(ReportSurfaceError::rejected("report_surface_options"));
        }
        let dispatch_storage = lease
            .claim(Self::DISPATCH_STORAGE_BYTES)
            .map_err(|cause| ReportSurfaceError::new("report_dispatch_credit", cause))?;
        Ok(Self { view, copy: messages(locale), lease, dispatch_storage })
    }
}
