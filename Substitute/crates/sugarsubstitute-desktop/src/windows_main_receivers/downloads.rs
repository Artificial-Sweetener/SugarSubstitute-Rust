// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve WebView2 download UI while retaining admitted operations through checked teardown.

pub(super) mod destination;

#[cfg(feature = "k1-native-qualification")]
pub(super) mod qualification;
use super::*;
use crate::main_downloads::{DownloadOperation, DownloadState, Downloads, admits_download};
use std::cell::RefCell;
use sugarsubstitute_contracts::configuration::LoopbackOrigin;

#[derive(Clone)]
struct NativeDownload(Rc<ICoreWebView2DownloadOperation>);

impl DownloadOperation for NativeDownload {
    fn state(&self) -> Result<DownloadState, DesktopError> {
        let mut state = COREWEBVIEW2_DOWNLOAD_STATE::default();
        // SAFETY: The live operation and writable outputs belong to this creating STA.
        unsafe {
            self.0
                .State(&mut state)
                .map_err(|cause| DesktopError::new("observe_browser_download", cause))?;
            let reason = if state == COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED {
                let mut reason = COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON::default();
                self.0
                    .InterruptReason(&mut reason)
                    .map_err(|cause| DesktopError::new("observe_browser_download", cause))?;
                Some(reason)
            } else {
                None
            };
            classify(state, reason)
        }
    }

    fn cancel(&self) -> Result<(), DesktopError> {
        // SAFETY: This exact retained operation belongs to the live creating STA.
        unsafe { self.0.Cancel() }
            .map_err(|cause| DesktopError::new("cancel_browser_download", cause))
    }
}

/// Adopt every partial COM reference before interpreting its getter result.
fn native_operation(
    args: &ICoreWebView2DownloadStartingEventArgs,
) -> (Option<NativeDownload>, Result<(), DesktopError>) {
    let mut raw = std::ptr::null_mut();
    // SAFETY: Exact live callback args and writable output. A returned owned reference
    // remains held even on failed HRESULT, before any cancellation/retirement decision.
    let status = unsafe { (args.vtable().DownloadOperation)(args.as_raw(), &mut raw) };
    let operation = if raw.is_null() {
        None
    } else {
        // SAFETY: Adopt the one returned COM reference, including a partial failed output.
        Some(NativeDownload(Rc::new(unsafe { ICoreWebView2DownloadOperation::from_raw(raw) })))
    };
    (operation, status.ok().map_err(|cause| DesktopError::new("get_browser_download", cause)))
}

/// Keep paused and automatically resumable interruptions under the same cancellation owner.
fn classify(
    state: COREWEBVIEW2_DOWNLOAD_STATE,
    reason: Option<COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON>,
) -> Result<DownloadState, DesktopError> {
    match (state, reason) {
        (COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED, _)
        | (
            COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED,
            Some(COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_CANCELED),
        ) => Ok(DownloadState::Terminal),
        (COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS, _)
        | (COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED, Some(_)) => Ok(DownloadState::Retained),
        _ => {
            Err(DesktopError::message("observe_browser_download", "unknown native download state"))
        }
    }
}

/// Bounded creating-thread metadata, charged by the existing receiver memory claim.
/// Native browser/profile ownership retains paths and bytes; Rust never opens destinations.
pub(super) struct Owner {
    origin: LoopbackOrigin,
    operations: Downloads<NativeDownload>,
    urls: RefCell<[String; 2]>,
    #[cfg(feature = "k1-native-qualification")]
    witness: RefCell<Option<Rc<qualification::DownloadWitness>>>,
}

impl Owner {
    /// Allocate fixed URL scratch storage before installing the native registration.
    pub(super) fn new(origin: LoopbackOrigin) -> Result<Rc<Self>, DesktopError> {
        let mut urls = [String::new(), String::new()];
        for value in &mut urls {
            value
                .try_reserve_exact(crate::main_navigation::MAX_URL_BYTES)
                .map_err(|cause| DesktopError::new("allocate_download_urls", cause))?;
            if value.capacity() != crate::main_navigation::MAX_URL_BYTES {
                return Err(DesktopError::message(
                    "allocate_download_urls",
                    "download URL capacity exceeded",
                ));
            }
        }
        Ok(Rc::new(Self {
            origin,
            operations: Downloads::new(),
            urls: RefCell::new(urls),
            #[cfg(feature = "k1-native-qualification")]
            witness: RefCell::new(None),
        }))
    }

    /// Bind one fixed observer before any qualification download; no observer owns cancellation.
    #[cfg(feature = "k1-native-qualification")]
    pub(super) fn witness(&self) -> Result<Rc<qualification::DownloadWitness>, DesktopError> {
        if self.witness.borrow().is_some() {
            return Err(DesktopError::message("qualify_downloads", "observer already attached"));
        }
        let witness = qualification::DownloadWitness::new()?;
        *self.witness.borrow_mut() = Some(witness.clone());
        Ok(witness)
    }

    /// Cancel by default, then admit only actual displayed-origin HTTP/blob downloads.
    /// Getters return before scratch/operation borrows; native dialog/path defaults remain.
    pub(super) fn starting(
        &self,
        sender: Option<&ICoreWebView2>,
        args: Option<&ICoreWebView2DownloadStartingEventArgs>,
        revealed: bool,
        events: &Events,
    ) -> Result<(), DesktopError> {
        let args = args.ok_or_else(|| {
            self.operations.retain_unknown_native_effect();
            DesktopError::new("browser_download_input", windows::core::Error::from(E_POINTER))
        })?;
        // SAFETY: Live callback args belong to this STA. Event cancellation suppresses
        // destination effects and the native dialog unless complete admission succeeds.
        self.operations.cancel_unadmitted(
            || {
                // SAFETY: Exact live callback args on the creating STA; no owner borrow is held.
                unsafe { args.SetCancel(true) }
                    .map_err(|cause| DesktopError::new("cancel_unadmitted_download", cause))
            },
            || native_operation(args),
        )?;
        if !revealed || !events.accepting() {
            return Ok(());
        }
        self.operations.reap_terminal()?;
        let sender = sender.ok_or_else(|| {
            DesktopError::new("browser_download_input", windows::core::Error::from(E_POINTER))
        })?;
        let (operation, status) = native_operation(args);
        status?;
        let operation = operation.ok_or_else(|| {
            DesktopError::new("get_browser_download", windows::core::Error::from(E_POINTER))
        })?;
        let mut source = PWSTR::null();
        // SAFETY: Exact live core, writable owned output, and no RefCell borrow held.
        let status = unsafe { sender.Source(&mut source) };
        let source = NativeString(source);
        status.map_err(|cause| DesktopError::new("get_download_document", cause))?;
        let mut uri = PWSTR::null();
        // SAFETY: Exact live operation, writable owned output, and no RefCell borrow held.
        let status = unsafe { operation.0.Uri(&mut uri) };
        let uri = NativeString(uri);
        status.map_err(|cause| DesktopError::new("get_download_uri", cause))?;
        if source.0.is_null() || uri.0.is_null() {
            return Err(DesktopError::new(
                "browser_download_input",
                windows::core::Error::from(E_POINTER),
            ));
        }
        let admitted = {
            let mut urls = self.urls.borrow_mut();
            for (target, native) in urls.iter_mut().zip([&source, &uri]) {
                target.clear();
                crate::main_events::copy_utf16(target, |index| {
                    // SAFETY: Successful getter supplies owned NUL-terminated UTF-16 data.
                    unsafe { native.0.as_ptr().add(index).read() }
                })?;
            }
            admits_download(self.origin, revealed, events.accepting(), &urls[0], &urls[1])
        };
        if !admitted {
            tracing::debug!(
                operation = "reject_browser_download",
                reason = "origin",
                "Frontend download rejected"
            );
            return Ok(());
        }
        #[cfg(feature = "k1-native-qualification")]
        let observed = operation.0.clone();
        if !self.operations.retain(operation)? {
            tracing::warn!(
                operation = "reject_browser_download",
                reason = "capacity",
                "Browser download capacity exhausted; cancel an existing download before retrying"
            );
            return Ok(());
        }
        if events.accepting() {
            // SAFETY: The bounded owner retained cancellation custody before browser admission.
            // Leave Handled and ResultFilePath untouched for the browser's normal download UI.
            unsafe { args.SetCancel(false) }
                .map_err(|cause| DesktopError::new("admit_browser_download", cause))?;
            #[cfg(feature = "k1-native-qualification")]
            if let Some(witness) = self.witness.borrow().as_ref() {
                witness.admitted(observed);
            }
            tracing::debug!(operation = "admit_browser_download", "Frontend download admitted");
        }
        Ok(())
    }

    /// Retire admitted operations before removing download receivers or closing the controller.
    pub(super) fn retire_step(&self) -> Result<bool, DesktopError> {
        self.operations.retire_step()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_paused_and_automatically_resumable_downloads_keep_cancellation_custody() {
        for reason in [
            COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_PAUSED,
            COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_SERVER_NO_RANGE,
            COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_HASH_MISMATCH,
            COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_FILE_TOO_SHORT,
        ] {
            assert!(matches!(
                classify(COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED, Some(reason)),
                Ok(DownloadState::Retained)
            ));
        }
    }

    #[test]
    fn native_completion_and_cancellation_retire_but_unknown_state_does_not() {
        assert!(matches!(
            classify(COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED, None),
            Ok(DownloadState::Terminal)
        ));
        assert!(matches!(
            classify(
                COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED,
                Some(COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_CANCELED)
            ),
            Ok(DownloadState::Terminal)
        ));
        assert!(classify(COREWEBVIEW2_DOWNLOAD_STATE(99), None).is_err());
        assert!(classify(COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED, None).is_err());
    }
}
