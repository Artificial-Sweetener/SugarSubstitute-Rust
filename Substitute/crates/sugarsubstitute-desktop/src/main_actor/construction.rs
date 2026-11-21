// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt every window, presenter, controller and native getter before interpreting outcomes.

use super::*;
use crate::{AccessibleControl, MainEnvironmentFacts, windows_controls::PaintMode};
use tao::{
    dpi::LogicalSize,
    window::{Theme, WindowBuilder},
};
use wry::{
    ConstructionProgress, WebContext, WebViewBuilder, WebViewBuilderExtWindows, WebViewExtWindows,
};

impl MainActor {
    /// Observe actual native visibility after both retained show operations return.
    pub(super) fn confirm_visible(&self) -> Result<(), DesktopError> {
        use tao::platform::windows::WindowExtWindows;
        use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::IsWindowVisible};
        let parent = HWND(self.window()?.hwnd() as *mut std::ffi::c_void);
        let mut visible = windows::core::BOOL::default();
        // SAFETY: Both exact retained objects belong to this UI thread; the scalar output is live.
        unsafe {
            self.view()?
                .controller()
                .IsVisible(&mut visible)
                .map_err(|cause| DesktopError::new("observe_main_visibility", cause))?;
            if !visible.as_bool() || !IsWindowVisible(parent).as_bool() {
                return Err(DesktopError::message(
                    "observe_main_visibility",
                    "main native surfaces remain hidden",
                ));
            }
        }
        Ok(())
    }
    pub(super) fn construct_step(
        &mut self,
        target: &EventLoopWindowTarget<NativeAction>,
        proxy: EventLoopProxy<NativeAction>,
        title: &str,
    ) -> Result<Option<DesktopStartupObservation>, DesktopError> {
        match self.stage {
            ConstructionStage::Window => {
                let settings = self.request()?.settings();
                let builder = WindowBuilder::new()
                    .with_title(title)
                    .with_inner_size(LogicalSize::new(
                        settings.logical_width,
                        settings.logical_height,
                    ))
                    .with_min_inner_size(LogicalSize::new(
                        crate::main_placement::MINIMUM_SIZE[0],
                        crate::main_placement::MINIMUM_SIZE[1],
                    ))
                    .with_decorations(false)
                    .with_resizable(true)
                    .with_transparent(true)
                    .with_theme(Some(if settings.dark { Theme::Dark } else { Theme::Light }))
                    .with_visible(false);
                // The composed browser has no HWND; its forwarding target owns OLE drops here.
                #[cfg(feature = "native-composition")]
                let builder = tao::platform::windows::WindowBuilderExtWindows::with_drag_and_drop(
                    builder, false,
                );
                self.window = Some(Rc::new(
                    builder
                        .build(target)
                        .map_err(|cause| DesktopError::new("create_main_window", cause))?,
                ));
                self.stage = ConstructionStage::Presenter;
            }
            ConstructionStage::Presenter => {
                self.presenter = Some(Presenter::new_main(
                    self.window()?.clone(),
                    self.request()?.settings().dark,
                )?);
                self.restore_placement()?;
                self.material = self.presenter.as_ref().map(Presenter::material);
                self.stage = ConstructionStage::Controls;
            }
            ConstructionStage::Controls => {
                let controls = self.caption_controls()?;
                self.controls = Some(Controls::new(
                    self.window()?.clone(),
                    proxy,
                    &crate::windows_controls::Specification::buttons(&controls),
                    PaintMode::NativeButtons(self.identity()),
                    None,
                )?);
                self.controls
                    .as_ref()
                    .ok_or_else(|| {
                        DesktopError::message("configure_main_caption", "retained controls absent")
                    })?
                    .configure_caption(
                        self.request()?.settings().dark,
                        self.material.ok_or_else(|| {
                            DesktopError::message(
                                "configure_main_caption",
                                "native material absent",
                            )
                        })?,
                    );
                self.controls
                    .as_mut()
                    .ok_or_else(|| {
                        DesktopError::message(
                            "initialize_main_controls",
                            "retained controls absent",
                        )
                    })?
                    .initialize()?;
                self.resize_caption()?;
                self.show_replacement_frame()?;
                #[cfg(feature = "native-composition")]
                {
                    self.stage = ConstructionStage::Hosting;
                }
                #[cfg(not(feature = "native-composition"))]
                {
                    self.stage = ConstructionStage::Begin;
                }
            }
            #[cfg(feature = "native-composition")]
            ConstructionStage::Hosting => {
                if self.select_hosting()? {
                    self.stage = ConstructionStage::Begin;
                }
            }
            ConstructionStage::Begin => {
                let mut context =
                    WebContext::new(Some(self.request()?.profile_directory().to_owned()));
                let captured = proxy;
                let builder = main_webview_builder(&mut context, self.browser_bounds()?);
                // DevTools is on in every launch: F12 opens it, and the browser serves it on
                // its loopback remote-debugging port.
                let builder = builder.with_devtools(true);
                // A windowed rebuild reuses the environment the parent already authorized.
                #[cfg(feature = "native-composition")]
                let builder = match self.reused_environment.take() {
                    Some(environment) => builder.with_environment(environment),
                    None => builder,
                };
                let wake = move || {
                    if let Err(cause) = captured.send_event(NativeAction::Main) {
                        tracing::error!(operation="wake_main_construction", cause=%cause,
                                "Native construction wake failed");
                    }
                };
                let begun = match self.construction_hosting()? {
                    crate::MainHosting::Composition => {
                        builder.begin_owned_as_composition_child(self.window()?.as_ref(), wake)
                    }
                    crate::MainHosting::Windowed => {
                        builder.begin_owned_as_child(self.window()?.as_ref(), wake)
                    }
                };
                match begun {
                    Ok(constructor) => self.constructor = Some(constructor),
                    Err(failure) => {
                        let (cause, partial) = failure.into_parts();
                        self.partial = Some(partial);
                        return Err(DesktopError::new("begin_main_construction", cause));
                    }
                }
                self.stage = ConstructionStage::Native;
            }
            ConstructionStage::Native => {
                let progress = self
                    .constructor
                    .as_mut()
                    .ok_or_else(|| {
                        DesktopError::message("progress_main_construction", "constructor missing")
                    })?
                    .progress()
                    .map_err(|cause| DesktopError::new("progress_main_construction", cause))?;
                match progress {
                    ConstructionProgress::EnvironmentPrepared => {
                        let claim = self
                            .request()?
                            .storage_lease()
                            .claim(MainEnvironmentFacts::MAX_STORAGE_BYTES)
                            .map_err(|cause| DesktopError::new("claim_main_environment", cause))?;
                        let folder = self
                            .constructor
                            .as_mut()
                            .ok_or_else(|| {
                                DesktopError::message(
                                    "query_main_environment",
                                    "constructor missing",
                                )
                            })?
                            .user_data_folder()
                            .map_err(|cause| DesktopError::new("query_main_environment", cause))?;
                        if let Some(folder) = folder {
                            let facts = MainEnvironmentFacts::new(folder.into_utf16(), claim)
                                .map_err(|cause| {
                                    DesktopError::new("adopt_main_environment", cause)
                                })?;
                            self.admission.environment_observed()?;
                            self.stage = ConstructionStage::PausedEnvironment;
                            return Ok(Some(DesktopStartupObservation::EnvironmentPrepared {
                                identity: self.identity(),
                                facts,
                            }));
                        }
                    }
                    ConstructionProgress::Finished => {
                        let result = self
                            .constructor
                            .as_mut()
                            .and_then(WebViewConstruction::take_result)
                            .ok_or_else(|| {
                                DesktopError::message(
                                    "take_main_result",
                                    "completed native result missing",
                                )
                            })?;
                        self.constructor = None;
                        match result {
                            Ok(view) => self.view = Some(view),
                            Err(failure) => {
                                let (cause, partial) = failure.into_parts();
                                self.partial = Some(partial);
                                return Err(DesktopError::new("create_main_controller", cause));
                            }
                        }
                        #[cfg(feature = "native-composition")]
                        if self.hosting == super::hosting::HostingSelection::Binding {
                            self.stage = ConstructionStage::Bind;
                            return Ok(None);
                        }
                        return self.observe_controller();
                    }
                    ConstructionProgress::Pending | ConstructionProgress::Advanced => {}
                }
            }
            #[cfg(feature = "native-composition")]
            ConstructionStage::Bind => match self.bind_browser()? {
                super::composition::BindStep::Pending => {}
                super::composition::BindStep::Bound => return self.observe_controller(),
                super::composition::BindStep::Rebuild => self.stage = ConstructionStage::Rebuild,
            },
            #[cfg(feature = "native-composition")]
            ConstructionStage::Rebuild => {
                if self.retire_composition_browser()? {
                    self.stage = ConstructionStage::Begin;
                }
            }
            ConstructionStage::Browser => {
                #[cfg(feature = "native-composition")]
                if self.hosting().presents_native_canvas() {
                    self.attach_composition_input()?;
                }
                if !self.hosting().presents_native_canvas() {
                    self.attach_windowed_focus()?;
                }
                if self.receivers.is_none() {
                    if self.session.is_none()
                        && let Some(bootstrap) = self.request()?.session_bootstrap()
                    {
                        self.session = Some(super::session::Channel::new(
                            self.identity(),
                            bootstrap.clone(),
                            self.view()?.webview(),
                            self.events()?.clone(),
                            self.request()?.storage_lease(),
                        )?);
                    }
                    self.receivers = Some(crate::windows_main_receivers::Receivers::new(
                        self.view()?.webview(),
                        self.events()?.clone(),
                        self.request()?.storage_lease(),
                        self.request()?.origin(),
                        self.bulk.clone(),
                        self.session.as_ref().and_then(super::session::Channel::media),
                    )?);
                } else if self
                    .receivers
                    .as_mut()
                    .ok_or_else(|| {
                        DesktopError::message("install_main_receivers", "receiver owner missing")
                    })?
                    .install_next()?
                {
                    self.stage = ConstructionStage::Live;
                    self.receivers()?.call(Command::Runtime, "Runtime.enable", "{}")?;
                    self.receivers()?.call(Command::Page, "Page.enable", "{}")?;
                }
            }
            ConstructionStage::PausedEnvironment | ConstructionStage::Live => {}
        }
        Ok(None)
    }

    /// Register focus reporting and Tab wrapping on a windowed browser once; a partial
    /// registration stays retained for retirement.
    fn attach_windowed_focus(&mut self) -> Result<(), DesktopError> {
        if self.windowed_focus.is_none() {
            self.windowed_focus = Some(focus::Focus::new(self.view()?.controller()));
        }
        self.windowed_focus
            .as_ref()
            .ok_or_else(|| DesktopError::message("windowed_browser_focus", "focus owner absent"))?
            .install()
    }

    /// Report the constructed browser's process exactly once, after its hosting is fixed.
    fn observe_controller(&mut self) -> Result<Option<DesktopStartupObservation>, DesktopError> {
        self.resize()?;
        let core = self.view()?.webview();
        crate::windows_main_receivers::configure_download_destination(
            &core,
            self.request()?.profile_directory(),
        )?;
        let mut pid = 0;
        // SAFETY: This UI thread owns the live controller; output is adopted as data.
        unsafe { core.BrowserProcessId(&mut pid) }
            .map_err(|cause| DesktopError::new("observe_main_browser", cause))?;
        self.admission.controller_observed(pid)?;
        self.stage = ConstructionStage::PausedEnvironment;
        Ok(Some(DesktopStartupObservation::ControllerPrepared {
            identity: self.identity(),
            browser_process_id: pid,
        }))
    }
    /// Return the hosting the next browser construction uses.
    fn construction_hosting(&self) -> Result<crate::MainHosting, DesktopError> {
        #[cfg(feature = "native-composition")]
        {
            self.hosting.construction().ok_or_else(|| {
                DesktopError::message("begin_main_construction", "browser hosting is undecided")
            })
        }
        #[cfg(not(feature = "native-composition"))]
        {
            Ok(crate::MainHosting::Windowed)
        }
    }
    /// The browser fills the client area, in exact physical pixels, so no logical round trip
    /// through a changing DPI can misalign it with the native canvas beneath it.
    pub(super) fn browser_bounds(&self) -> Result<wry::Rect, DesktopError> {
        let size = self.window()?.inner_size();
        Ok(super::frame::browser_bounds(size.width, size.height))
    }
    /// The caption's logical layout width: the client's, never narrower than its buttons need.
    fn caption_width(&self) -> Result<f64, DesktopError> {
        let window = self.window()?;
        Ok(window.inner_size().to_logical::<f64>(window.scale_factor()).width.max(144.0))
    }
    pub(super) fn caption_controls(
        &self,
    ) -> Result<[AccessibleControl; crate::main_caption::PEERS], DesktopError> {
        let window = self.window()?;
        Ok(crate::main_caption::caption_controls(
            self.caption_width()?,
            window.is_maximized(),
            self.request()?.settings().labels,
        ))
    }
    /// Lay the caption out for the current client size and fill the client with the browser.
    /// Moving and resizing the browser keeps its page, scroll and state.
    pub(super) fn resize(&mut self) -> Result<(), DesktopError> {
        self.resize_caption()?;
        let bounds = self.browser_bounds()?;
        if let Some(view) = &self.view {
            view.set_bounds(bounds)
                .map_err(|cause| DesktopError::new("resize_main_view", cause))?;
        }
        #[cfg(feature = "native-composition")]
        self.place_composed_browser()?;
        if let Some(owner) = &self.controls {
            owner.raise_main()?;
        }
        #[cfg(feature = "native-composition")]
        self.withdraw_stale_canvas()?;
        Ok(())
    }

    /// Follow a window DPI change. The composed browser takes the new scale before its bounds
    /// change; the windowed browser follows its monitor itself.
    pub(super) fn rescale(
        &mut self,
        #[cfg_attr(not(feature = "native-composition"), allow(unused_variables))] scale: f64,
    ) -> Result<(), DesktopError> {
        #[cfg(feature = "native-composition")]
        self.rescale_composition(scale)?;
        self.resize()
    }
}

/// The browser argument that opens DevTools on its fixed loopback port in every launch.
pub(crate) const MAIN_DEVTOOLS_ARGUMENT: &str = "--remote-debugging-port=9333";

/// The main browser as owned construction admits it: hidden, unfocused, transparent, blank,
/// serving DevTools on its loopback port, without download or other generic handlers. Owned
/// construction refuses any wry callback handler; the desktop's own receivers observe browser
/// events once the browser exists, new windows included.
fn main_webview_builder(context: &mut WebContext, bounds: wry::Rect) -> WebViewBuilder<'_> {
    WebViewBuilder::new_with_web_context(context)
        .with_url("about:blank")
        .with_visible(false)
        .with_focused(false)
        .with_transparent(true)
        .with_additional_browser_args(MAIN_DEVTOOLS_ARGUMENT)
        .without_download_handlers()
        .with_bounds(bounds)
}

#[cfg(test)]
mod builder_tests {
    use super::*;
    use wry::raw_window_handle::{
        HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle,
    };

    /// A parent the owned builder only reads: validation precedes every window or COM effect.
    struct Parent;
    impl HasWindowHandle for Parent {
        fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
            let handle = std::num::NonZeroIsize::new(1).ok_or(HandleError::Unavailable)?;
            // SAFETY: The handle is never dereferenced: owned construction validates its whole
            // configuration before using the parent window.
            Ok(unsafe {
                WindowHandle::borrow_raw(RawWindowHandle::Win32(Win32WindowHandle::new(handle)))
            })
        }
    }

    /// Owned construction admits the main browser's configuration in both hostings.
    #[test]
    fn owned_construction_admits_the_main_browser_configuration() {
        let profile = std::env::temp_dir().join("sugarsubstitute-main-builder-profile");
        for composition in [false, true] {
            let mut context = WebContext::new(Some(profile.clone()));
            let builder = main_webview_builder(&mut context, wry::Rect::default());
            let begun = if composition {
                builder.begin_owned_as_composition_child(&Parent, || {})
            } else {
                builder.begin_owned_as_child(&Parent, || {})
            };
            assert!(begun.is_ok(), "composition={composition}");
        }
    }
}
