// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Forward OLE drags over the host window to the composed browser.
//!
//! A composition-hosted WebView2 owns no HWND, so the host registers the window's only
//! `IDropTarget` and forwards every phase to `ICoreWebView2CompositionController3` in browser
//! client pixels. The window is built without Tao's own drop target for this reason.

use super::callbacks::{CallbackFence, retire_when_quiet};
use super::*;
use std::cell::{Cell, RefCell};
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2CompositionController3;
use windows::{
    Win32::{
        Foundation::{DRAGDROP_E_INVALIDHWND, DRAGDROP_E_NOTREGISTERED, E_POINTER, POINT, POINTL},
        Graphics::Gdi::ClientToScreen,
        System::{
            Com::IDataObject,
            Ole::{
                DROPEFFECT, DROPEFFECT_NONE, DROPEFFECT_SCROLL, IDropTarget, IDropTarget_Impl,
                OleInitialize, OleUninitialize, RegisterDragDrop, RevokeDragDrop,
            },
            SystemServices::MODIFIERKEYS_FLAGS,
        },
    },
    core::{Ref, implement},
};

/// OLE passes the source's allowed effects in; the browser's choice must stay within them.
/// The scroll bit is a target-only indicator that OLE accepts with any permitted effect.
fn effect_within(allowed: DROPEFFECT, chosen: DROPEFFECT) -> DROPEFFECT {
    DROPEFFECT(chosen.0 & (allowed.0 | DROPEFFECT_SCROLL.0))
}

/// Convert an OLE screen point to browser client pixels.
/// The browser fills the client area from its origin, as for forwarded mouse input.
fn browser_point(screen: POINTL, client_origin: POINT) -> Option<POINT> {
    Some(POINT {
        x: screen.x.checked_sub(client_origin.x)?,
        y: screen.y.checked_sub(client_origin.y)?,
    })
}

/// One positioned drag phase and the data object OLE lends for its duration.
enum Phase<'a> {
    Enter(Option<&'a IDataObject>),
    Over,
    Drop(Option<&'a IDataObject>),
}

/// Forwarding state shared with OLE's target reference on the window's UI thread.
struct State {
    window: HWND,
    controller: RefCell<Option<ICoreWebView2CompositionController3>>,
    events: Rc<Events>,
    active: Cell<bool>,
    entered: Cell<bool>,
    /// This window is the source of the current OS drag, which the page must not receive.
    exporting: Cell<bool>,
    callbacks: CallbackFence,
}

impl State {
    /// Forward one positioned phase and always write a permitted effect for OLE.
    fn forward(
        &self,
        phase: Phase<'_>,
        keys: MODIFIERKEYS_FLAGS,
        screen: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        if effect.is_null() {
            return Err(E_POINTER.into());
        }
        // SAFETY: OLE lends a live, aligned in/out effect for the duration of this callback.
        let allowed = unsafe { effect.read() };
        let chosen = self.deliver(phase, keys, screen, allowed).unwrap_or_else(|cause| {
            self.events.fail(cause);
            DROPEFFECT_NONE
        });
        // SAFETY: The same lent effect remains live until this callback returns.
        unsafe { effect.write(chosen) };
        Ok(())
    }

    fn deliver(
        &self,
        phase: Phase<'_>,
        keys: MODIFIERKEYS_FLAGS,
        screen: &POINTL,
        allowed: DROPEFFECT,
    ) -> Result<DROPEFFECT, DesktopError> {
        let entering = matches!(phase, Phase::Enter(_));
        let continuing = entering || self.entered.get();
        if matches!(phase, Phase::Drop(_)) {
            self.entered.set(false);
        }
        if !continuing || !self.active.get() || self.exporting.get() || !self.events.accepting() {
            return Ok(DROPEFFECT_NONE);
        }
        let _callback = self.events.callback();
        let _scope = self.callbacks.enter(&self.events)?;
        let Some(controller) = self.controller.borrow().clone() else {
            return Ok(DROPEFFECT_NONE);
        };
        let mut origin = POINT::default();
        // SAFETY: This UI thread owns the live HWND; the native call borrows local storage.
        if !unsafe { ClientToScreen(self.window, &raw mut origin) }.as_bool() {
            return Err(DesktopError::message(
                "browser_drag_coordinates",
                "Native client origin is unavailable",
            ));
        }
        let point = browser_point(*screen, origin).ok_or_else(|| {
            DesktopError::message("browser_drag_coordinates", "Drag point is out of range")
        })?;
        let mut chosen = allowed.0;
        // SAFETY: The cloned controller is used on its creating UI apartment; OLE lends the data
        // object for this callback and the effect output is live local storage.
        let result = unsafe {
            match phase {
                Phase::Enter(data) => controller.DragEnter(data, keys.0, point, &raw mut chosen),
                Phase::Over => controller.DragOver(keys.0, point, &raw mut chosen),
                Phase::Drop(data) => controller.Drop(data, keys.0, point, &raw mut chosen),
            }
        };
        if entering {
            self.entered.set(result.is_ok());
        }
        result.map_err(|cause| DesktopError::new("forward_browser_drag", cause))?;
        Ok(effect_within(allowed, DROPEFFECT(chosen)))
    }

    /// End a forwarded drag that the browser accepted on entry.
    fn leave(&self) {
        if !self.entered.replace(false) || !self.active.get() || !self.events.accepting() {
            return;
        }
        let _callback = self.events.callback();
        let _scope = match self.callbacks.enter(&self.events) {
            Ok(scope) => scope,
            Err(cause) => return self.events.fail(cause),
        };
        let Some(controller) = self.controller.borrow().clone() else { return };
        // SAFETY: The cloned controller is used on its creating UI apartment.
        if let Err(cause) = unsafe { controller.DragLeave() } {
            self.events.fail(DesktopError::new("forward_browser_drag_leave", cause));
        }
    }
}

/// OLE's reference to the forwarding state; never agile, because the state is UI-affine.
#[implement(IDropTarget, Agile = false)]
struct Target(Rc<State>);

impl IDropTarget_Impl for Target_Impl {
    fn DragEnter(
        &self,
        data: Ref<IDataObject>,
        keys: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        self.0.forward(Phase::Enter(data.as_ref()), keys, point, effect)
    }

    fn DragOver(
        &self,
        keys: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        self.0.forward(Phase::Over, keys, point, effect)
    }

    fn DragLeave(&self) -> windows::core::Result<()> {
        self.0.leave();
        Ok(())
    }

    fn Drop(
        &self,
        data: Ref<IDataObject>,
        keys: MODIFIERKEYS_FLAGS,
        point: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        self.0.forward(Phase::Drop(data.as_ref()), keys, point, effect)
    }
}

/// Native effects of one HWND drop registration, separated so their ordering is testable.
trait Registrar {
    /// Acquire one OLE reference on the current STA thread.
    fn initialize(&mut self) -> windows::core::Result<()>;
    /// Register the target; OLE retains it until revocation or window destruction.
    fn register(&mut self) -> windows::core::Result<()>;
    /// Revoke the registration, returning OLE's target reference.
    fn revoke(&mut self) -> windows::core::Result<()>;
    /// Balance exactly one successful initialization on the same thread.
    fn uninitialize(&mut self);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Stage {
    Registered,
    Revoked,
    Released,
}

/// Order registration teardown: revoke, release the forwarded controller, then balance OLE.
struct Lifecycle<R: Registrar> {
    registrar: R,
    stage: Stage,
}

impl<R: Registrar> Lifecycle<R> {
    /// Initialize OLE and register; a rejected registration balances OLE before returning.
    fn open(mut registrar: R) -> Result<Self, DesktopError> {
        registrar
            .initialize()
            .map_err(|cause| DesktopError::new("initialize_browser_drop_ole", cause))?;
        if let Err(cause) = registrar.register() {
            registrar.uninitialize();
            return Err(DesktopError::new("register_browser_drop_target", cause));
        }
        Ok(Self { registrar, stage: Stage::Registered })
    }

    /// A failed revocation retains the controller and OLE reference for a later retry.
    fn retire(&mut self, release: impl FnOnce()) -> Result<(), DesktopError> {
        if self.stage == Stage::Registered {
            settled(self.registrar.revoke())
                .map_err(|cause| DesktopError::new("revoke_browser_drop_target", cause))?;
            self.stage = Stage::Revoked;
        }
        if self.stage == Stage::Revoked {
            release();
            self.registrar.uninitialize();
            self.stage = Stage::Released;
        }
        Ok(())
    }
}

impl<R: Registrar> Drop for Lifecycle<R> {
    fn drop(&mut self) {
        if self.stage != Stage::Released {
            tracing::error!(
                operation = "browser_drop_target_unresolved_drop",
                "Retaining the browser drop registration and OLE reference until thread exit"
            );
        }
    }
}

/// Tao revokes every registration while destroying its window, leaving no target reference.
fn settled(result: windows::core::Result<()>) -> windows::core::Result<()> {
    match result {
        Err(cause)
            if cause.code() == DRAGDROP_E_NOTREGISTERED
                || cause.code() == DRAGDROP_E_INVALIDHWND =>
        {
            Ok(())
        }
        other => other,
    }
}

/// The host window's OLE registration on its creating STA thread.
struct Native {
    window: HWND,
    target: IDropTarget,
}

impl Registrar for Native {
    fn initialize(&mut self) -> windows::core::Result<()> {
        // SAFETY: The UI thread is an STA; S_FALSE still acquires one balanced reference.
        unsafe { OleInitialize(None) }
    }

    fn register(&mut self) -> windows::core::Result<()> {
        // SAFETY: This thread owns the live HWND and OLE retains its own target reference.
        unsafe { RegisterDragDrop(self.window, &self.target) }
    }

    fn revoke(&mut self) -> windows::core::Result<()> {
        // SAFETY: This thread registered the target on this HWND.
        unsafe { RevokeDragDrop(self.window) }
    }

    fn uninitialize(&mut self) {
        // SAFETY: Balances this owner's one successful OleInitialize on the same thread.
        unsafe { OleUninitialize() }
    }
}

/// Own the host window's browser drop target from registration through OLE balance.
pub(super) struct DropTarget {
    state: Rc<State>,
    lifecycle: Lifecycle<Native>,
}

impl DropTarget {
    /// Register on the live host HWND after the composition controller is ready.
    /// Fails when the runtime lacks drag forwarding or another target owns the window.
    pub(super) fn register(
        window: HWND,
        controller: &ICoreWebView2CompositionController,
        events: Rc<Events>,
    ) -> Result<Self, DesktopError> {
        let controller = controller
            .cast::<ICoreWebView2CompositionController3>()
            .map_err(|cause| DesktopError::new("query_browser_drop_controller", cause))?;
        let state = Rc::new(State {
            window,
            controller: RefCell::new(Some(controller)),
            events,
            active: Cell::new(true),
            entered: Cell::new(false),
            exporting: Cell::new(false),
            callbacks: CallbackFence::default(),
        });
        let target: IDropTarget = Target(state.clone()).into();
        let lifecycle = Lifecycle::open(Native { window, target })?;
        Ok(Self { state, lifecycle })
    }

    /// Decline further drags before asynchronous main-window retirement.
    pub(super) fn suspend(&self) {
        self.state.active.set(false);
    }

    /// The handle a queued drag-out uses to make this window refuse its own export.
    pub(super) fn exporter(&self) -> Exporter {
        Exporter(self.state.clone())
    }

    /// Revoke only after every forwarded drag call returned; true once OLE is balanced.
    pub(super) fn retire(&mut self) -> Result<bool, DesktopError> {
        self.suspend();
        let state = &self.state;
        let lifecycle = &mut self.lifecycle;
        retire_when_quiet(&state.callbacks, || lifecycle.retire(|| drop(state.controller.take())))
    }
}

/// Refuse drops on this window while it is the source of an OS drag-out.
pub(super) struct Exporter(Rc<State>);

impl Exporter {
    /// Run `export` with this window's drops refused. Retirement waits until it returns,
    /// because the modal drag still uses the OLE apartment and registration this target owns.
    /// # Errors
    /// Fails without running `export` when the callback depth is exhausted.
    pub(super) fn export<T>(&self, export: impl FnOnce() -> T) -> Result<T, DesktopError> {
        let _scope = self.0.callbacks.enter(&self.0.events)?;
        self.0.exporting.set(true);
        let result = export();
        self.0.exporting.set(false);
        Ok(result)
    }
}

#[cfg(test)]
mod tests;
