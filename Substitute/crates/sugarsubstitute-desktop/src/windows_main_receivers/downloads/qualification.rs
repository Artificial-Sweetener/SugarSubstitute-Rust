// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe three production-admitted browser operations without controlling their lifecycle.

use super::super::*;
use std::{
    cell::{Cell, RefCell},
    path::Path,
};

/// Fixed observer references share the original production receiver's metadata claim.
/// Only the production Downloads owner issues cancellation or declares teardown settled.
pub(crate) struct DownloadWitness {
    operations: RefCell<[Option<Rc<ICoreWebView2DownloadOperation>>; 3]>,
    extra: Cell<bool>,
    scratch: RefCell<[String; 2]>,
}
impl DownloadWitness {
    /// Allocate bounded observation scratch before the one qualification script is submitted.
    pub(super) fn new() -> Result<Rc<Self>, DesktopError> {
        let mut scratch = [String::new(), String::new()];
        for value in &mut scratch {
            value
                .try_reserve_exact(4096)
                .map_err(|cause| DesktopError::new("observe_download_qualification", cause))?;
            if value.capacity() != 4096 {
                return Err(DesktopError::message(
                    "observe_download_qualification",
                    "scratch capacity",
                ));
            }
        }
        Ok(Rc::new(Self {
            operations: RefCell::new(std::array::from_fn(|_| None)),
            extra: Cell::new(false),
            scratch: RefCell::new(scratch),
        }))
    }
    /// Observe only after the production owner retained and actually admitted this operation.
    pub(super) fn admitted(&self, operation: Rc<ICoreWebView2DownloadOperation>) {
        let mut operations = self.operations.borrow_mut();
        if operations.iter().flatten().any(|prior| prior.as_raw() == operation.as_raw()) {
            self.extra.set(true);
        } else if let Some(slot) = operations.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(operation);
        } else {
            self.extra.set(true);
        }
    }
    /// Require three distinct native completions, exact destination parents and JSON/PNG paths.
    pub(crate) fn completed(
        &self,
        destination: &Path,
        origin: sugarsubstitute_contracts::configuration::LoopbackOrigin,
    ) -> Result<bool, DesktopError> {
        if self.extra.get() {
            return Err(DesktopError::message(
                "observe_download_qualification",
                "unexpected download",
            ));
        }
        let operations = self.operations.borrow().clone();
        if operations.iter().any(Option::is_none) {
            return Ok(false);
        }
        let mut kinds = 0_u8;
        for operation in operations.into_iter().flatten() {
            let mut state = COREWEBVIEW2_DOWNLOAD_STATE::default();
            // SAFETY: Read-only getters observe the exact retained UI-affine operation.
            unsafe { operation.State(&mut state) }
                .map_err(|cause| DesktopError::new("observe_download_qualification", cause))?;
            match state {
                COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS => return Ok(false),
                COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED => {}
                _ => {
                    return Err(DesktopError::message(
                        "observe_download_qualification",
                        "download did not complete",
                    ));
                }
            }
            let mut mime = PWSTR::null();
            // SAFETY: Adopt the owned native string even if its getter reports failure.
            let status = unsafe { operation.MimeType(&mut mime) };
            let mime = NativeString(mime);
            status.map_err(|cause| DesktopError::new("observe_download_qualification", cause))?;
            let mut uri = PWSTR::null();
            // SAFETY: Exact retained operation, writable owned output, no storage borrow.
            let status = unsafe { operation.Uri(&mut uri) };
            let uri = NativeString(uri);
            status.map_err(|cause| DesktopError::new("observe_download_qualification", cause))?;
            let mut path = PWSTR::null();
            // SAFETY: Exact retained operation, writable owned output, no storage borrow.
            let status = unsafe { operation.ResultFilePath(&mut path) };
            let path = NativeString(path);
            status.map_err(|cause| DesktopError::new("observe_download_qualification", cause))?;
            let kind = {
                let mut scratch = self.scratch.borrow_mut();
                copy(&mut scratch[0], &mime)?;
                let json = scratch[0] == "application/json";
                let png = scratch[0] == "image/png";
                copy(&mut scratch[0], &uri)?;
                if !crate::main_downloads::admits_download(
                    origin,
                    true,
                    true,
                    &crate::main_navigation::root_url(origin),
                    &scratch[0],
                ) {
                    return Err(DesktopError::message(
                        "observe_download_qualification",
                        "download origin changed",
                    ));
                }
                let blob = scratch[0].starts_with("blob:");
                copy(&mut scratch[1], &path)?;
                if Path::new(&scratch[1]).parent() != Some(destination) {
                    return Err(DesktopError::message(
                        "observe_download_qualification",
                        "destination changed",
                    ));
                }
                match (blob, json, png) {
                    (true, true, false) => 1,
                    (false, false, true) => 2,
                    (true, false, true) => 4,
                    _ => {
                        return Err(DesktopError::message(
                            "observe_download_qualification",
                            "download kind changed",
                        ));
                    }
                }
            };
            if kinds & kind != 0 {
                return Err(DesktopError::message(
                    "observe_download_qualification",
                    "duplicate download kind",
                ));
            }
            kinds |= kind;
        }
        Ok(kinds == 7)
    }
}

/// Decode only an already returned owned native string into fixed admitted scratch.
fn copy(target: &mut String, value: &NativeString) -> Result<(), DesktopError> {
    target.clear();
    if value.0.is_null() {
        return Err(DesktopError::new(
            "observe_download_qualification",
            windows::core::Error::from(E_POINTER),
        ));
    }
    crate::main_events::copy_utf16(target, |index| {
        // SAFETY: Successful getter supplies valid owned NUL-terminated UTF-16 data.
        unsafe { value.0.as_ptr().add(index).read() }
    })
}
