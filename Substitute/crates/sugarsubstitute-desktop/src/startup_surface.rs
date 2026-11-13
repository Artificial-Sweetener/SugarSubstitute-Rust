// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own accepted splash presentation independently of backend and main lifetimes.

use crate::timing::{NativeStartupStage, observe_startup};
use crate::{
    DesktopError, DesktopOptions, DesktopReceipt, WindowContent,
    native_action::NativeAction,
    native_surface::{presentation::Presentations, state::SurfaceState, surface_shape},
    windows::Presenter,
    windows_controls::{Controls, PaintMode},
};
use std::{rc::Rc, time::Instant};
use tao::{
    dpi::LogicalSize,
    event_loop::EventLoop,
    platform::windows::WindowExtWindows,
    window::{Theme, Window, WindowBuilder},
};
use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::DestroyWindow};

#[path = "startup_surface_custody.rs"]
mod custody;
use custody::{Initialization, SurfacePort};

/// Retain the complete splash native lifetime across presentation-only retirement and failures.
pub(super) struct Splash {
    pub(super) window: Option<Rc<Window>>,
    pub(super) presenter: Option<Presenter>,
    pub(super) controls: Option<Controls>,
    pub(super) state: SurfaceState,
    pub(super) presentations: Option<Presentations>,
    pub(super) receipt: Option<DesktopReceipt>,
    pub(super) retiring: bool,
    pub(super) retired: bool,
    pub(super) presented: bool,
    pub(super) launch: sugarsubstitute_contracts::startup::LaunchId,
    material: crate::NativeMaterial,
    initialization: Initialization,
}
impl Splash {
    /// Retain an inert surface owner before any native creation is attempted.
    pub(super) fn new(
        options: &DesktopOptions,
        content: &impl crate::StartupWindowContent,
        started: Instant,
        timings: Option<crate::timing::TimingObserver>,
    ) -> Self {
        Self {
            window: None,
            presenter: None,
            controls: None,
            state: SurfaceState::new(timings),
            presentations: Some(Presentations::new(started, options.audit_frames)),
            receipt: None,
            retiring: false,
            retired: false,
            presented: false,
            launch: content.launch_id(),
            material: crate::NativeMaterial::ThemedFallback,
            initialization: Initialization::new(),
        }
    }

    /// Create native slots once while the outer session owns this actual partial surface.
    pub(super) fn initialize(
        &mut self,
        options: &DesktopOptions,
        event_loop: &EventLoop<NativeAction>,
        content: &impl crate::StartupWindowContent,
        timings: Option<crate::timing::TimingObserver>,
    ) -> Result<(), DesktopError> {
        let mut port =
            SplashPort { options, event_loop, controls: content.accessible_controls(), timings };
        let result = self.initialization.initialize(
            &mut self.window,
            &mut self.presenter,
            &mut self.controls,
            &mut port,
        );
        self.material = self
            .presenter
            .as_ref()
            .map_or(crate::NativeMaterial::ThemedFallback, Presenter::material);
        result
    }
    /// Bind a retry to fresh pixels; an exposed prior-launch frame cannot submit this launch.
    pub(super) fn bind_launch(&mut self, launch: sugarsubstitute_contracts::startup::LaunchId) {
        if self.launch != launch && !self.retiring {
            self.launch = launch;
            self.presented = false;
            self.state.retained = None;
            self.state.dirty();
        }
    }
    pub(super) fn window(&self) -> Result<&Rc<Window>, DesktopError> {
        self.window
            .as_ref()
            .ok_or_else(|| DesktopError::message("startup_splash", "splash window retired"))
    }
    pub(super) fn shape(
        &self,
    ) -> Result<Option<sugarsubstitute_contracts::presentation::FrameShape>, DesktopError> {
        self.window.as_ref().map(|window| surface_shape(window)).transpose().map(Option::flatten)
    }
    pub(super) fn begin_retirement(
        &mut self,
        content: &mut impl WindowContent,
    ) -> Result<(), DesktopError> {
        if self.retiring || self.retired {
            return Ok(());
        }
        self.retiring = true;
        self.state.visibility(content, false);
        self.state.retained = None;
        if let Some(window) = &self.window {
            window.set_visible(false);
        }
        Ok(())
    }
    /// Perform one checked teardown phase; never cancel the backend through splash retirement.
    pub(super) fn retire_step(&mut self) -> Result<bool, DesktopError> {
        if self.retired {
            return Ok(true);
        }
        if let Some(controls) = &mut self.controls {
            controls.shutdown()?;
            if controls.retired() {
                self.controls = None;
            }
            return Ok(false);
        }
        if let Some(presenter) = &mut self.presenter {
            presenter.shutdown()?;
            self.presenter = None;
            return Ok(false);
        }
        if let Some(window) = &self.window {
            let hwnd = HWND(window.hwnd() as *mut std::ffi::c_void);
            // SAFETY: Controls and presenter have retired; exact parent stays held on this UI thread.
            unsafe { DestroyWindow(hwnd) }
                .map_err(|cause| DesktopError::new("destroy_startup_splash", cause))?;
            self.window = None;
            return Ok(false);
        }
        self.retired = true;
        let presentations = self.presentations.take().ok_or_else(|| {
            DesktopError::message("finish_splash_presentations", "presentation owner missing")
        })?;
        self.receipt = Some(presentations.finish(self.material)?);
        Ok(true)
    }
}
/// Borrow native inputs without owning or hiding any partial returned resource.
struct SplashPort<'a> {
    options: &'a DesktopOptions,
    event_loop: &'a EventLoop<NativeAction>,
    controls: &'a [crate::AccessibleControl],
    timings: Option<crate::timing::TimingObserver>,
}
impl SurfacePort for SplashPort<'_> {
    type Window = Rc<Window>;
    type Presenter = Presenter;
    type Controls = Controls;
    fn window(&mut self) -> Result<Self::Window, DesktopError> {
        Ok(Rc::new(
            WindowBuilder::new()
                .with_title(&self.options.title)
                .with_inner_size(LogicalSize::new(
                    self.options.logical_width,
                    self.options.logical_height,
                ))
                .with_decorations(false)
                .with_resizable(false)
                .with_transparent(true)
                .with_theme(Some(if self.options.dark { Theme::Dark } else { Theme::Light }))
                .with_visible(false)
                .build(self.event_loop)
                .map_err(|cause| DesktopError::new("create_startup_splash", cause))?,
        ))
    }
    fn window_adopted(&mut self) -> Result<(), DesktopError> {
        observe_startup(self.timings.as_ref(), NativeStartupStage::WindowCreated)
    }
    fn presenter(&mut self, window: &Self::Window) -> Result<Presenter, DesktopError> {
        Presenter::new(window.clone(), self.options.dark)
    }
    fn presenter_adopted(&mut self) -> Result<(), DesktopError> {
        observe_startup(self.timings.as_ref(), NativeStartupStage::PresenterReady)
    }
    fn controls(&mut self, window: &Self::Window) -> Result<Controls, DesktopError> {
        Controls::new(
            window.clone(),
            self.event_loop.create_proxy(),
            &crate::windows_controls::Specification::buttons(self.controls),
            PaintMode::PreparedPixels,
            self.timings.clone(),
        )
    }
    fn initialize_controls(&mut self, controls: &mut Controls) -> Result<(), DesktopError> {
        controls.initialize()
    }
    fn controls_adopted(&mut self) -> Result<(), DesktopError> {
        observe_startup(self.timings.as_ref(), NativeStartupStage::ControlsReady)
    }
}

impl Drop for Splash {
    fn drop(&mut self) {
        if !self.retired {
            std::mem::forget(self.controls.take());
            std::mem::forget(self.presenter.take());
            std::mem::forget(self.window.take());
            tracing::error!(
                operation = "startup_splash_unresolved_drop",
                "Retaining unresolved native splash lifetime until process termination"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        num::NonZeroUsize,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };
    use sugarsubstitute_contracts::{
        execution::{Generation, MemoryLease, MemoryRelease, SurfaceId},
        presentation::{FrameIdentity, FrameShape, FrameValidator},
        startup::LaunchId,
    };

    struct Release(Arc<AtomicUsize>);
    impl MemoryRelease for Release {
        fn release(&self, _: u64) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// A same-launch exposure preserves submission, but retry requires fresh successful pixels.
    #[test]
    fn retry_cannot_relabel_prior_splash_submission() -> Result<(), Box<dyn std::error::Error>> {
        let first = LaunchId::new(1)?;
        let next = LaunchId::new(2)?;
        let mut splash = Splash {
            window: None,
            presenter: None,
            controls: None,
            state: SurfaceState::new(None),
            presentations: None,
            receipt: None,
            retiring: false,
            retired: false,
            presented: true,
            launch: first,
            material: crate::NativeMaterial::ThemedFallback,
            initialization: Initialization::new(),
        };
        let released = Arc::new(AtomicUsize::new(0));
        let shape = FrameShape::new(1, 1, 1.0)?;
        let pixels = vec![0; shape.byte_len()];
        let identity = FrameIdentity {
            surface: SurfaceId::new(1)?,
            state_generation: Generation::new(1)?,
            sequence: Generation::new(1)?,
        };
        let lease = MemoryLease::new(pixels.capacity() as u64, Box::new(Release(released.clone())));
        let mut validator = FrameValidator::new(shape, identity, pixels, lease)?;
        assert!(validator.validate_next(NonZeroUsize::new(1).ok_or("zero fixture budget")?)?);
        splash.state.retained = Some(validator.finish()?);
        splash.bind_launch(first);
        assert!(splash.presented);
        assert!(splash.state.retained.is_some());
        assert_eq!(released.load(Ordering::SeqCst), 0);
        splash.bind_launch(next);
        assert_eq!(splash.launch, next);
        assert!(!splash.presented);
        assert!(splash.state.retained.is_none());
        assert_eq!(released.load(Ordering::SeqCst), 1);
        splash.presented = true;
        splash.retiring = true;
        splash.bind_launch(first);
        assert_eq!(splash.launch, next);
        assert!(splash.presented);
        // No native slots exist in this fixture; retirement still cannot invent a frame receipt.
        assert!(splash.retire_step().is_err());
        assert!(splash.retired);
        Ok(())
    }
}
