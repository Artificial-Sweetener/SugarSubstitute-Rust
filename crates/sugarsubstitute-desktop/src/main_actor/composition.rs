// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain a browser visual tree on the main UI apartment through checked teardown.

mod callbacks;
mod canvas;
mod construction;
mod cursor;
mod drop_target;
mod input;
mod retirement;
mod routing;

use super::*;
use retirement::FencedCommit;
use std::ffi::c_void;
use sugarsubstitute_contracts::execution::MemoryClaim;
use tao::platform::windows::WindowExtWindows;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2CompositionController, ICoreWebView2Controller,
};
use windows::{
    Win32::{
        Foundation::{E_POINTER, HWND},
        Graphics::DirectComposition::{
            IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
        },
    },
    core::{GUID, HRESULT, IUnknown, Interface},
};
use wry::WebViewExtWindows;

windows::core::link!("dcomp.dll" "system" fn DCompositionCreateDevice2(
    rendering_device: *mut c_void,
    iid: *const GUID,
    result: *mut *mut c_void,
) -> HRESULT);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Stage {
    Device,
    Target,
    Root,
    Browser,
    Tree,
    Child,
    Controller,
    Bind,
    Commit,
    Ready,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Retire {
    DetachBrowser,
    DetachTree,
    Commit,
    Release,
}

/// Produce the DirectComposition device; tests substitute a deterministic native failure.
#[derive(Clone, Copy, Debug)]
pub(super) enum DeviceSource {
    /// The system compositor, which fails under unsupported sessions or drivers.
    System,
    /// A native creation failure with this status and no transferred device.
    #[cfg(test)]
    Failing(HRESULT),
}

impl DeviceSource {
    /// Request one device, writing any transferred reference into `raw` before reporting status.
    fn create(self, raw: &mut *mut c_void) -> HRESULT {
        match self {
            // SAFETY: The requested IID and writable output match the adopted interface.
            Self::System => unsafe {
                DCompositionCreateDevice2(std::ptr::null_mut(), &IDCompositionDevice::IID, raw)
            },
            #[cfg(test)]
            Self::Failing(status) => status,
        }
    }
}

/// Preserve uncertainty when a native mutation fails after potentially taking effect.
#[derive(Default)]
struct Ledger {
    browser_bound: bool,
    tree_bound: bool,
    dirty: bool,
    committed: bool,
}

impl Ledger {
    fn next(&self) -> Retire {
        if self.browser_bound {
            Retire::DetachBrowser
        } else if self.tree_bound {
            Retire::DetachTree
        } else if self.dirty {
            Retire::Commit
        } else {
            Retire::Release
        }
    }

    /// Advance only after the exact native effect succeeds; failed effects remain retryable.
    fn step<E>(&mut self, effect: impl FnOnce(Retire) -> Result<(), E>) -> Result<bool, E> {
        let action = self.next();
        if action == Retire::Release {
            return Ok(true);
        }
        effect(action)?;
        match action {
            Retire::DetachBrowser => {
                self.browser_bound = false;
                self.dirty = true;
            }
            Retire::DetachTree => {
                self.tree_bound = false;
                self.dirty = true;
            }
            Retire::Commit => {
                self.dirty = false;
                self.committed = true;
            }
            Retire::Release => {}
        }
        Ok(false)
    }
}

/// This owner's original HWND, controller, COM outputs and memory claim remain UI-affine.
pub(super) struct Composition {
    source: DeviceSource,
    window: Option<Rc<Window>>,
    controller: Option<ICoreWebView2Controller>,
    composition: Option<ICoreWebView2CompositionController>,
    device: Option<IDCompositionDevice>,
    target: Option<IDCompositionTarget>,
    root: Option<IDCompositionVisual>,
    browser: Option<IDCompositionVisual>,
    canvas: Option<canvas::Canvas>,
    retirement_fence: Option<FencedCommit>,
    input: Option<input::Input>,
    claim: Option<MemoryClaim>,
    stage: Stage,
    ledger: Ledger,
    retired: bool,
}

impl Composition {
    /// Retain the parent and claim before any native composition effect.
    /// The browser controller is attached only after the composition surface exists.
    fn new(window: Rc<Window>, claim: MemoryClaim, source: DeviceSource) -> Self {
        Self {
            source,
            window: Some(window),
            controller: None,
            composition: None,
            device: None,
            target: None,
            root: None,
            browser: None,
            canvas: None,
            retirement_fence: None,
            input: None,
            claim: Some(claim),
            stage: Stage::Device,
            ledger: Ledger::default(),
            retired: false,
        }
    }

    /// Run one bounded native effect and retain every output before checking its status.
    fn advance(&mut self) -> Result<bool, DesktopError> {
        match self.stage {
            Stage::Device => {
                let mut raw = std::ptr::null_mut();
                let status = self.source.create(&mut raw);
                // SAFETY: The call requested IDCompositionDevice, including on partial failure.
                unsafe { adopt(&mut self.device, raw, status, "create_composition_device")? };
                self.stage = Stage::Target;
            }
            Stage::Target => {
                let device = self.device()?;
                let window = self.window.as_ref().ok_or_else(|| missing("composition_window"))?;
                let hwnd = HWND(window.hwnd() as *mut c_void);
                let mut raw = std::ptr::null_mut();
                // SAFETY: This apartment owns the device and exact HWND; the output is writable.
                let status = unsafe {
                    (device.vtable().CreateTargetForHwnd)(
                        device.as_raw(),
                        hwnd,
                        true.into(),
                        &mut raw,
                    )
                };
                // SAFETY: This native method transfers an IDCompositionTarget reference.
                unsafe { adopt(&mut self.target, raw, status, "create_composition_target")? };
                self.stage = Stage::Root;
            }
            Stage::Root | Stage::Browser => {
                let device = self.device()?;
                let mut raw = std::ptr::null_mut();
                // SAFETY: This retained device writes one owned IDCompositionVisual output.
                let status = unsafe { (device.vtable().CreateVisual)(device.as_raw(), &mut raw) };
                let root = self.stage == Stage::Root;
                let destination = if root { &mut self.root } else { &mut self.browser };
                // SAFETY: CreateVisual transfers an IDCompositionVisual reference.
                unsafe { adopt(destination, raw, status, "create_composition_visual")? };
                self.stage = if root { Stage::Browser } else { Stage::Tree };
            }
            Stage::Tree => {
                self.ledger.tree_bound = true;
                self.ledger.dirty = true;
                // SAFETY: The exact retained target and root belong to this UI apartment.
                unsafe { self.target()?.SetRoot(self.root()?) }
                    .map_err(|cause| DesktopError::new("bind_composition_tree", cause))?;
                self.stage = Stage::Child;
            }
            Stage::Child => {
                // SAFETY: Both visuals belong to the retained device and stay live through commit.
                unsafe {
                    self.root()?.AddVisual(self.browser()?, true, None::<&IDCompositionVisual>)
                }
                .map_err(|cause| DesktopError::new("add_browser_visual", cause))?;
                self.stage = Stage::Controller;
            }
            Stage::Controller => {
                let controller =
                    self.controller.as_ref().ok_or_else(|| missing("browser_controller"))?;
                let mut raw = std::ptr::null_mut();
                // SAFETY: QueryInterface receives the exact IID and one writable transferred output.
                let status = unsafe {
                    (controller.vtable().base__.QueryInterface)(
                        controller.as_raw(),
                        &ICoreWebView2CompositionController::IID,
                        &mut raw,
                    )
                };
                // SAFETY: The requested IID identifies the adopted composition controller.
                unsafe {
                    adopt(&mut self.composition, raw, status, "query_composition_controller")?
                };
                self.stage = Stage::Bind;
            }
            Stage::Bind => {
                self.ledger.browser_bound = true;
                self.ledger.dirty = true;
                let controller = self.composition()?;
                let browser = self.browser()?;
                // SAFETY: IDCompositionVisual derives from IUnknown and both objects remain retained.
                let status = unsafe {
                    (controller.vtable().SetRootVisualTarget)(controller.as_raw(), browser.as_raw())
                };
                status.ok().map_err(|cause| DesktopError::new("bind_browser_visual", cause))?;
                self.stage = Stage::Commit;
            }
            Stage::Commit => {
                // SAFETY: This exact device owns the tree and the synchronous commit request.
                unsafe { self.device()?.Commit() }
                    .map_err(|cause| DesktopError::new("commit_browser_visual", cause))?;
                self.ledger.dirty = false;
                self.ledger.committed = true;
                self.stage = Stage::Ready;
            }
            Stage::Ready => return Ok(true),
        }
        Ok(false)
    }

    /// Detach the browser and target, commit removal, then release on the creating apartment.
    pub(super) fn retire_step(&mut self) -> Result<bool, DesktopError> {
        if self.retired {
            return Ok(true);
        }
        if let Some(input) = &mut self.input {
            if input.retire()? {
                self.input = None;
            }
            return Ok(false);
        }
        if let Some(canvas) = &mut self.canvas {
            canvas.end_draws()?;
            canvas.reap_retirements()?;
            if canvas.has_retirements() {
                return Ok(false);
            }
        }
        let controller = &self.composition;
        let target = &self.target;
        let device = &self.device;
        let released = self.ledger.step(|action| -> Result<(), DesktopError> {
            match action {
                Retire::DetachBrowser => {
                    // SAFETY: The controller is retained and null disconnects its browser visual.
                    unsafe {
                        controller
                            .as_ref()
                            .ok_or_else(|| missing("composition_controller"))?
                            .SetRootVisualTarget(None::<&IUnknown>)
                    }
                    .map_err(|cause| DesktopError::new("detach_browser_visual", cause))?;
                    Ok(())
                }
                Retire::DetachTree => {
                    // SAFETY: This target remains live until its committed root has been cleared.
                    unsafe {
                        target
                            .as_ref()
                            .ok_or_else(|| missing("composition_target"))?
                            .SetRoot(None::<&IDCompositionVisual>)
                    }
                    .map_err(|cause| DesktopError::new("detach_composition_tree", cause))?;
                    Ok(())
                }
                Retire::Commit => {
                    // SAFETY: The device outlives every outstanding visual-tree mutation.
                    unsafe {
                        device.as_ref().ok_or_else(|| missing("composition_device"))?.Commit()
                    }
                    .map_err(|cause| DesktopError::new("commit_composition_retirement", cause))?;
                    Ok(())
                }
                Retire::Release => Ok(()),
            }
        })?;
        if released {
            if self.ledger.committed {
                if self.retirement_fence.is_none() {
                    self.retirement_fence = Some(FencedCommit::new(self.device()?.clone()));
                }
                let fence = self
                    .retirement_fence
                    .as_ref()
                    .ok_or_else(|| missing("composition_retirement_fence"))?;
                fence.check()?;
                if !fence.completed() {
                    return Ok(false);
                }
            }
            if let Some(canvas) = &mut self.canvas {
                canvas.release_after_detach()?;
            }
            self.retirement_fence = None;
            self.canvas = None;
            self.browser = None;
            self.root = None;
            self.target = None;
            self.device = None;
            self.composition = None;
            self.controller = None;
            self.window = None;
            self.claim = None;
            self.retired = true;
        }
        Ok(released)
    }

    /// Report whether the device, target and visual tree exist and await the browser.
    fn surface_ready(&self) -> bool {
        self.stage == Stage::Controller
    }

    /// Adopt the composition-hosted browser controller once its surface exists.
    fn attach_controller(
        &mut self,
        controller: ICoreWebView2Controller,
    ) -> Result<(), DesktopError> {
        if self.controller.is_some() || !self.surface_ready() {
            return Err(DesktopError::message(
                "attach_browser_controller",
                "composition surface is not awaiting a browser",
            ));
        }
        self.controller = Some(controller);
        Ok(())
    }

    fn device(&self) -> Result<&IDCompositionDevice, DesktopError> {
        self.device.as_ref().ok_or_else(|| missing("composition_device"))
    }
    fn target(&self) -> Result<&IDCompositionTarget, DesktopError> {
        self.target.as_ref().ok_or_else(|| missing("composition_target"))
    }
    fn root(&self) -> Result<&IDCompositionVisual, DesktopError> {
        self.root.as_ref().ok_or_else(|| missing("composition_root"))
    }
    fn browser(&self) -> Result<&IDCompositionVisual, DesktopError> {
        self.browser.as_ref().ok_or_else(|| missing("composition_browser"))
    }
    fn composition(&self) -> Result<&ICoreWebView2CompositionController, DesktopError> {
        self.composition.as_ref().ok_or_else(|| missing("composition_controller"))
    }

    /// Admit pointer, cursor and drag hosting only after the browser visual is committed.
    fn prepare_input(&mut self, events: Rc<Events>) -> Result<(), DesktopError> {
        if self.input.is_none() {
            let window = self.window.as_ref().ok_or_else(|| missing("composition_window"))?;
            let hwnd = HWND(window.hwnd() as *mut c_void);
            self.input = Some(input::Input::new(hwnd, self.composition()?.clone(), events)?);
        }
        self.input.as_mut().ok_or_else(|| missing("composition_input"))?.attach()
    }

    /// Close input admission before the browser and visual tree begin retirement.
    pub(super) fn suspend_input(&self) {
        if let Some(input) = &self.input {
            input.suspend();
        }
    }
}

impl Drop for Composition {
    fn drop(&mut self) {
        if !self.retired {
            std::mem::forget(self.window.take());
            std::mem::forget(self.controller.take());
            std::mem::forget(self.composition.take());
            std::mem::forget(self.device.take());
            std::mem::forget(self.target.take());
            std::mem::forget(self.root.take());
            std::mem::forget(self.browser.take());
            std::mem::forget(self.canvas.take());
            std::mem::forget(self.retirement_fence.take());
            std::mem::forget(self.input.take());
            std::mem::forget(self.claim.take());
            tracing::error!(
                operation = "browser_composition_unresolved_drop",
                "Retaining UI-affine composition until process termination"
            );
        }
    }
}

/// Adopt the transferred COM reference even if its producing native call reports failure.
unsafe fn adopt<T: Interface>(
    slot: &mut Option<T>,
    raw: *mut c_void,
    status: HRESULT,
    operation: &'static str,
) -> Result<(), DesktopError> {
    if !raw.is_null() {
        // SAFETY: The caller requests the exact T interface and transfers its returned reference.
        *slot = Some(unsafe { T::from_raw(raw) });
    }
    status.ok().map_err(|cause| DesktopError::new(operation, cause))?;
    if slot.is_none() {
        return Err(DesktopError::new(operation, windows::core::Error::from(E_POINTER)));
    }
    Ok(())
}

fn missing(operation: &'static str) -> DesktopError {
    DesktopError::message(operation, "retained composition resource missing")
}

#[cfg(test)]
mod tests;
