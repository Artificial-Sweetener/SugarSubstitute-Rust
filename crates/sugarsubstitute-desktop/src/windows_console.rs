// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain a standard read-only RichEdit peer through checked native retirement.
//! The exact presented frame admits the bounded document before any native effect.
//! Win32 owns shaping, selection, word wrapping, scrolling, keyboard copy and WM_GETOBJECT.
use super::{Creation, NativePort, PaintMode, focus_window};
use crate::{
    ConsoleView, DesktopError,
    console::{funding::Funding, index::DocumentIndex, update::Documents},
    native_action::NativeAction,
};
use std::rc::Rc;
use sugarsubstitute_contracts::presentation::PreparedFrame;
use tao::{event_loop::EventLoopProxy, window::Window};
use windows::Win32::UI::Input::KeyboardAndMouse::GetFocus;
#[path = "windows_console/accessibility.rs"]
mod accessibility;
#[path = "windows_console/callback.rs"]
mod callback;
#[path = "windows_console/engine.rs"]
mod engine;
#[path = "windows_console/messages.rs"]
mod messages;
#[path = "windows_console/native.rs"]
mod native;
#[path = "windows_console/render_proxy.rs"]
mod render_proxy;
#[path = "windows_console/text.rs"]
mod text;
use native::{Library, Peer, Port};
/// No native child can outlive its original document funding or callback custody.
pub(super) struct Console {
    creation: Creation<Library, Peer>,
    callback: Rc<callback::Callback>,
    documents: Option<Documents>,
    visible: bool,
    bounds: Option<(crate::ControlBounds, f64)>,
    healthy: bool,
    funding: Funding,
}
impl Console {
    /// Claim/copy first; the containing Controls slot adopts this inert owner before initialize.
    pub(super) fn new(
        proxy: EventLoopProxy<NativeAction>,
        timings: Option<crate::timing::TimingObserver>,
        frame: &PreparedFrame,
        view: &ConsoleView<'_>,
    ) -> Result<Self, DesktopError> {
        let (funding, claim) = Funding::new(frame)?;
        let pending = match view.text {
            Some(source) => DocumentIndex::prepare(source, claim)?,
            None => DocumentIndex::empty(claim)?,
        };
        let callback = Rc::new(callback::Callback::new(proxy, timings));
        Ok(Self {
            creation: Creation::new(1)?,
            callback,
            documents: Some(Documents::new(pending)),
            visible: false,
            bounds: None,
            healthy: true,
            funding,
        })
    }
    /// Each partial native return is retained by the existing Creation transaction.
    pub(super) fn initialize(
        &mut self,
        window: &Window,
        view: &ConsoleView<'_>,
    ) -> Result<(), DesktopError> {
        let mut port = Port {
            window,
            callback: &self.callback,
            label: view.label,
            bounds: Some(view.bounds),
            module: self.creation.parent.as_ref().and_then(Library::module),
        };
        self.creation.initialize(&mut port)?;
        self.bounds = Some((view.bounds, window.scale_factor()));
        Ok(())
    }
    /// Publish current data only after complete copying, formatting and selection restoration.
    pub(super) fn present(
        &mut self,
        window: &Window,
        frame: &PreparedFrame,
        view: &ConsoleView<'_>,
    ) -> Result<(), DesktopError> {
        if !self.healthy || !self.creation.writable() {
            return Err(DesktopError::message(
                "console_present",
                "console is incomplete, failed or closing",
            ));
        }
        let documents = self
            .documents
            .as_mut()
            .ok_or_else(|| DesktopError::message("console_present", "document custody absent"))?;
        if let Some(source) = view.text {
            let same = documents.current().is_some_and(|old| {
                old.resource == Some(source.resource()) && old.revision == source.revision()
            });
            if documents.current().is_some_and(|old| {
                old.resource == Some(source.resource()) && old.revision > source.revision()
            }) {
                return Err(DesktopError::message(
                    "console_present",
                    "console source revision regressed",
                ));
            }
            if !same
                && !documents.pending()
                && let Some(claim) = self.funding.next(frame)?
            {
                documents.stage(DocumentIndex::prepare(source, claim)?)?;
            }
        }
        let peer = self
            .creation
            .peers
            .first()
            .ok_or_else(|| DesktopError::message("console_present", "retained peer absent"))?;
        let result = (|| {
            peer.value.live()?;
            let document_changed = documents.pending();
            text::publish(peer.value.hwnd, documents, view.dark)?;
            let rect = crate::control_geometry::physical_bounds(
                &view.native_control(),
                window.scale_factor(),
            )?;
            peer.value.engine.prepare(
                frame,
                documents.shared().ok_or_else(|| {
                    DesktopError::message("console_present", "published source absent")
                })?,
                rect,
                view.dark,
                window.scale_factor(),
            )?;
            let bounds = (view.bounds, window.scale_factor());
            let layout_changed = self.bounds != Some(bounds) || self.visible != view.visible;
            let reflow = if self.bounds != Some(bounds) {
                Some(text::observe(peer.value.hwnd, view.dark)?)
            } else {
                None
            };
            peer.value.live()?;
            if layout_changed {
                native::present(window, peer.value.hwnd, &self.callback, view)?;
            }
            peer.value.live()?;
            if let Some(observation) = reflow
                && let Some(document) = documents.current()
            {
                text::reflow(peer.value.hwnd, document, observation, view.dark)?;
            }
            peer.value.live()?;
            if document_changed || layout_changed {
                native::repaint(peer.value.hwnd, view.visible)?;
            }
            peer.value.live()?;
            self.bounds = Some(bounds);
            self.visible = view.visible;
            Ok(())
        })();
        if result.is_err() {
            self.healthy = false;
            self.callback.active.set(false);
        }
        result
    }
    pub(super) fn hwnd(&self) -> Option<windows::Win32::Foundation::HWND> {
        self.visible.then(|| self.creation.peers.first().map(|p| p.value.hwnd)).flatten()
    }
    /// Return layout ordering only while this exact native text peer is available.
    pub(super) fn order_y(&self) -> Option<f64> {
        self.visible.then(|| self.bounds.map(|(bounds, _)| bounds.y)).flatten()
    }
    /// Close actions before native teardown; failed closes keep both document claims.
    pub(super) fn shutdown(&mut self, window: &Window) -> Result<(), DesktopError> {
        self.callback.active.set(false);
        let mut port = Port {
            window,
            callback: &self.callback,
            label: "",
            bounds: None,
            module: self.creation.parent.as_ref().and_then(Library::module),
        };
        self.creation.shutdown(&mut port)?;
        Ok(())
    }
    pub(super) fn retired(&self) -> bool {
        self.creation.retired(Rc::strong_count(&self.callback) == 1)
    }
}
impl Drop for Console {
    fn drop(&mut self) {
        if !self.retired() {
            self.creation.retain_unresolved();
            std::mem::forget(self.callback.clone());
            std::mem::forget(self.documents.take());
            tracing::error!(
                operation = "console_unresolved_drop",
                "Retaining native console and original document funding until process termination"
            );
        }
    }
}

impl super::Controls {
    /// Reconcile a distinct read-only native document only against this actual presented frame.
    pub(crate) fn present_console(
        &mut self,
        frame: &sugarsubstitute_contracts::presentation::PreparedFrame,
        view: Option<crate::ConsoleView<'_>>,
    ) -> Result<(), DesktopError> {
        let Some(view) = view else {
            return Ok(());
        };
        if self.planned.iter().any(|control| control.id == 4) {
            return Err(DesktopError::message(
                "console_present",
                "native console ID4 collides with a planned peer",
            ));
        }
        native::validate(&view, self.window.scale_factor())?;
        if self.paint_mode != PaintMode::PreparedPixels || !self.creation.writable() {
            return Err(DesktopError::message(
                "console_present",
                "original splash controls unavailable",
            ));
        }
        if self.console.is_none() && !view.visible {
            return Ok(());
        }
        if self.console.is_none() {
            self.console =
                Some(Console::new(self.callback.proxy(), self.callback.timings(), frame, &view)?);
            self.console
                .as_mut()
                .ok_or_else(|| {
                    DesktopError::message("console_adopt", "native console owner absent")
                })?
                .initialize(&self.window, &view)?;
        }
        if !view.visible
            && self.console.as_ref().and_then(Console::hwnd).is_some_and(|hwnd| {
                // SAFETY: Compare exact retained child to this UI thread's passive focus observation.
                (unsafe { GetFocus() }) == hwnd
            })
            && let Some(disclosure) = self
                .creation
                .peers
                .iter()
                .map(|slot| &slot.value)
                .find(|peer| peer.snapshot.id == 1 && peer.snapshot.visible)
        {
            focus_window(disclosure.hwnd)?;
        }
        if let Some(console) = self.console.as_mut() {
            console.present(&self.window, frame, &view)?;
        }
        Ok(())
    }
}
