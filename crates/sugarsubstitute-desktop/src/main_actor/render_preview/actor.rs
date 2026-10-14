// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Seal genuine current main facts immediately before one optional native renderer capture.

use super::{Capture, capture::current_ticks};
use crate::qualification_trace::{NativeFailure, NativePhase, native_failed};
use crate::{
    DesktopError, DesktopStartupObservation, MainIdentity, MainPreviewSnapshot,
    MainPreviewViewport, main_actor::MainActor, main_readiness_types::DocumentId,
};
use sugarsubstitute_contracts::execution::CounterDeadline;
use tao::platform::windows::WindowExtWindows;
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2;
use windows::Win32::{
    Foundation::{HWND, RECT},
    UI::HiDpi::GetDpiForWindow,
};
use wry::WebViewExtWindows;

/// The fields are constructor-private to the actual MainActor's native observation path.
pub(super) struct MainRenderSeal {
    snapshot: MainPreviewSnapshot,
    core: ICoreWebView2,
}
impl MainRenderSeal {
    /// Transfer genuine native source facts to their same-thread capture owner.
    pub(super) fn into_parts(self) -> (MainPreviewSnapshot, ICoreWebView2) {
        (self.snapshot, self.core)
    }
}

impl MainActor {
    /// Admit one explicit qualification command without changing product readiness policy.
    pub(crate) fn capture_preview(
        &mut self,
        identity: MainIdentity,
        source: CounterDeadline,
    ) -> Option<DesktopStartupObservation> {
        if self.preview_requested {
            return Some(self.preview_failure(
                NativePhase::Admission,
                DesktopError::message(
                    "capture_main_preview",
                    "qualification capture already requested",
                ),
            ));
        }
        self.preview_requested = true;
        let seal = match self.render_seal(identity, source) {
            Ok(seal) => seal,
            Err(cause) => return Some(self.preview_failure(NativePhase::Seal, cause)),
        };
        let created =
            (|| Capture::new(seal, self.request()?.storage_lease(), self.events()?.clone()))();
        match created {
            Ok(owner) => self.preview = Some(owner),
            Err(cause) => return Some(self.preview_failure(NativePhase::Storage, cause)),
        }
        let issued = self
            .preview
            .as_mut()
            .ok_or_else(|| DesktopError::message("capture_main_preview", "adopted owner missing"))
            .and_then(Capture::issue);
        match issued {
            Ok(()) => None,
            Err(cause) => Some(self.preview_failure(NativePhase::Issue, cause)),
        }
    }

    /// Create only genuine same-thread current facts, with no filesystem or hash work.
    fn render_seal(
        &self,
        identity: MainIdentity,
        source: CounterDeadline,
    ) -> Result<MainRenderSeal, DesktopError> {
        self.admission.frontend(identity)?;
        self.preview_quiescence()?;
        if !self.revealed || self.retired {
            return Err(DesktopError::message("capture_main_preview", "main not revealed"));
        }
        self.confirm_visible()?;
        let document =
            self.readiness.as_ref().and_then(|owner| owner.render_document()).ok_or_else(|| {
                DesktopError::message("capture_main_preview", "current document missing")
            })?;
        let native_navigation = self.receivers()?.preview_navigation()?;
        let core = self.view()?.webview();
        let controller = self.view()?.controller();
        let mut browser_pid = 0;
        let mut rectangle = RECT::default();
        // SAFETY: Exact retained interfaces belong to this creating STA, and Windows
        // writes only these live stack scalar outputs before the immutable seal is made.
        unsafe {
            core.BrowserProcessId(&mut browser_pid)
                .map_err(|cause| DesktopError::new("observe_preview_browser", cause))?;
            controller
                .Bounds(&mut rectangle)
                .map_err(|cause| DesktopError::new("observe_preview_bounds", cause))?;
        }
        if browser_pid == 0 || self.admission.browser_process_id() != Some(browser_pid) {
            return Err(DesktopError::message(
                "observe_preview_browser",
                "current browser changed",
            ));
        }
        let width = u32::try_from(i64::from(rectangle.right) - i64::from(rectangle.left))
            .map_err(|cause| DesktopError::new("observe_preview_bounds", cause))?;
        let height = u32::try_from(i64::from(rectangle.bottom) - i64::from(rectangle.top))
            .map_err(|cause| DesktopError::new("observe_preview_bounds", cause))?;
        if width == 0 || height == 0 || u64::from(width) * u64::from(height) > 8_388_608 {
            return Err(DesktopError::message("observe_preview_bounds", "viewport pixel bound"));
        }
        // SAFETY: The actual owned main HWND remains retained on its creating thread;
        // this getter observes DPI without changing bounds, activation, focus or input.
        let dpi = unsafe { GetDpiForWindow(HWND(self.window()?.hwnd() as *mut _)) };
        if dpi == 0 {
            return Err(DesktopError::message("observe_preview_dpi", "native DPI missing"));
        }
        let versions = self.request()?.versions();
        let versions = [
            DocumentId::new(versions.frontend())
                .map_err(|cause| DesktopError::new("observe_preview_versions", cause))?,
            DocumentId::new(versions.sugarcubes())
                .map_err(|cause| DesktopError::new("observe_preview_versions", cause))?,
        ];
        self.preview_quiescence()?;
        if self.receivers()?.preview_navigation()? != native_navigation
            || self.readiness.as_ref().and_then(|owner| owner.render_document()) != Some(document)
        {
            return Err(DesktopError::message("capture_main_preview", "native document changed"));
        }
        Ok(MainRenderSeal {
            snapshot: MainPreviewSnapshot {
                document,
                native_navigation,
                browser_pid,
                viewport: MainPreviewViewport {
                    left: rectangle.left,
                    top: rectangle.top,
                    width,
                    height,
                    dpi,
                },
                origin: self.request()?.origin(),
                versions,
                source_deadline: source,
                issued_qpc: current_ticks(source)?,
            },
            core,
        })
    }

    /// Publish only after real completion and current source/viewport/document revalidation.
    pub(in crate::main_actor) fn preview_progress(&mut self) -> Option<DesktopStartupObservation> {
        let pending = self.preview.as_ref()?;
        if !pending.publication_pending() {
            return None;
        }
        let source = pending.snapshot().source_deadline();
        let current = match self.render_seal(self.identity(), source) {
            Ok(seal) => seal,
            Err(cause) => return Some(self.preview_failure(NativePhase::Revalidate, cause)),
        };
        if let Err(cause) = self.preview_quiescence() {
            return Some(self.preview_failure(NativePhase::Revalidate, cause));
        }
        let owner = self.preview.as_mut()?;
        if !owner.matches(current.snapshot, &current.core) {
            return Some(self.preview_failure(
                NativePhase::Revalidate,
                DesktopError::message(
                    "complete_main_preview",
                    "captured native document or viewport changed",
                ),
            ));
        }
        match owner.take() {
            Ok(Some(image)) => Some(DesktopStartupObservation::MainPreviewCaptured(image)),
            Ok(None) => None,
            Err(cause) => Some(self.preview_failure(NativePhase::Publication, cause)),
        }
    }

    fn preview_failure(
        &mut self,
        phase: NativePhase,
        cause: DesktopError,
    ) -> DesktopStartupObservation {
        if let Some(owner) = &mut self.preview {
            owner.reported_failure();
        }
        native_failed(self.identity(), NativeFailure::from_error(phase, &cause));
        DesktopStartupObservation::MainPreviewFailed { identity: self.identity(), cause }
    }

    /// Native getters may pump callbacks; publication waits for the existing health queue.
    fn preview_quiescence(&self) -> Result<(), DesktopError> {
        if self.events()?.preview_quiescent() {
            Ok(())
        } else {
            Err(DesktopError::message("capture_main_preview", "native health queue not quiescent"))
        }
    }
}
