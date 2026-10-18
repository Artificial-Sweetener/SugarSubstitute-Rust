// SPDX-License-Identifier: GPL-3.0-or-later
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
                self.window = Some(Rc::new(
                    WindowBuilder::new()
                        .with_title(title)
                        .with_inner_size(LogicalSize::new(
                            settings.logical_width,
                            settings.logical_height,
                        ))
                        .with_min_inner_size(LogicalSize::new(304, 240))
                        .with_decorations(false)
                        .with_resizable(true)
                        .with_transparent(true)
                        .with_theme(Some(if settings.dark { Theme::Dark } else { Theme::Light }))
                        .with_visible(false)
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
                self.material = self.presenter.as_ref().map(Presenter::material);
                self.stage = ConstructionStage::Controls;
            }
            ConstructionStage::Controls => {
                let controls = self.caption_controls()?;
                self.controls = Some(Controls::new(
                    self.window()?.clone(),
                    proxy,
                    &controls,
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
                self.present_recovery_control()?;
                self.stage = ConstructionStage::Begin;
            }
            ConstructionStage::Begin => {
                let mut context =
                    WebContext::new(Some(self.request()?.profile_directory().to_owned()));
                let captured = proxy;
                let begun = WebViewBuilder::new_with_web_context(&mut context)
                    .with_url("about:blank")
                    .with_visible(false)
                    .with_focused(false)
                    .with_transparent(true)
                    .with_additional_browser_args("")
                    .without_download_handlers()
                    .with_bounds(self.browser_bounds()?)
                    .begin_owned_as_child(self.window()?.as_ref(), move || {
                        if let Err(cause) = captured.send_event(NativeAction::Main) {
                            tracing::error!(operation="wake_main_construction", cause=%cause,
                                "Native construction wake failed");
                        }
                    });
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
                        self.resize()?;
                        let core = self.view()?.webview();
                        let mut pid = 0;
                        // SAFETY: This UI thread owns the live controller; output is adopted as data.
                        unsafe { core.BrowserProcessId(&mut pid) }
                            .map_err(|cause| DesktopError::new("observe_main_browser", cause))?;
                        self.admission.controller_observed(pid)?;
                        self.stage = ConstructionStage::PausedEnvironment;
                        return Ok(Some(DesktopStartupObservation::ControllerPrepared {
                            identity: self.identity(),
                            browser_process_id: pid,
                        }));
                    }
                    ConstructionProgress::Pending | ConstructionProgress::Advanced => {}
                }
            }
            ConstructionStage::Browser => {
                if self.receivers.is_none() {
                    self.receivers = Some(crate::windows_main_receivers::Receivers::new(
                        self.view()?.webview(),
                        self.events()?.clone(),
                        self.request()?.storage_lease(),
                        self.request()?.origin(),
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

    pub(super) fn browser_bounds(&self) -> Result<wry::Rect, DesktopError> {
        let window = self.window()?;
        let size = window.inner_size().to_logical::<f64>(window.scale_factor());
        Ok(super::frame::browser_bounds(size.width, size.height))
    }
    pub(super) fn caption_controls(&self) -> Result<[AccessibleControl; 4], DesktopError> {
        let window = self.window()?;
        let width = window.inner_size().to_logical::<f64>(window.scale_factor()).width.max(144.0);
        let settings = self.request()?.settings();
        Ok(crate::main_backend::caption_controls(
            width,
            window.is_maximized(),
            settings.labels,
            self.recovery.presentation(settings.recovery),
        ))
    }
    pub(super) fn resize(&mut self) -> Result<(), DesktopError> {
        let controls = self.caption_controls()?;
        if let Some(owner) = &mut self.controls {
            owner.update(&controls)?;
        }
        if let Some(view) = &self.view {
            view.set_bounds(self.browser_bounds()?)
                .map_err(|cause| DesktopError::new("resize_main_view", cause))?;
        }
        if let Some(owner) = &self.controls {
            owner.raise_main()?;
        }
        Ok(())
    }
}
