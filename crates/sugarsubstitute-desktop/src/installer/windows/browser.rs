// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own one private browser attempt through checked construction and actual collection retirement.
use super::*;
use tao::{event_loop::EventLoopProxy, window::Window};
use wry::PrivateProfileObserver;

#[derive(Default)]
pub(super) struct Browser {
    context: Option<WebContext>,
    constructor: Option<WebViewConstruction>,
    partial: Option<WebViewRetirement>,
    pub(super) view: Option<WebView>,
    observer: Option<PrivateProfileObserver>,
    observed: bool,
    progress: Option<ConstructionProgress>,
    started: bool,
    deadline: Option<Instant>,
}
impl Browser {
    /// Build only after an execution-owned storage job persisted the private profile grant.
    pub(super) fn advance(
        &mut self,
        lifetime: &mut impl InstallerLifetime,
        window: &Window,
        proxy: &EventLoopProxy<NativeAction>,
    ) -> Result<bool, Box<dyn Error>> {
        if !self.started {
            let Some(profile) = lifetime.profile_path() else {
                return Ok(false);
            };
            self.context = Some(WebContext::new(Some(profile)));
            let proxy = proxy.clone();
            let context = self.context.as_mut().ok_or("Installer browser context absent")?;
            let builder = WebViewBuilder::new_with_web_context(context)
                .with_url("about:blank")
                .with_visible(false)
                .with_focused(false)
                .with_additional_browser_args("--disk-cache-size=33554432")
                .with_exclusive_user_data_folder_access(true)
                .without_download_handlers()
                .with_bounds(wry::Rect {
                    position: tao::dpi::PhysicalPosition::new(0, 0).into(),
                    size: window.inner_size().into(),
                })
                .with_transparent(true)
                .with_scroll_bar_style(wry::ScrollBarStyle::FluentOverlay);
            self.started = true;
            match builder.begin_owned_as_child(window, move || {
                if let Err(cause) = proxy.send_event(NativeAction::Main) {
                    tracing::error!(operation="installer_construction", cause=%cause, "Installer wake failed");
                }
            }) {
                Ok(value) => self.constructor = Some(value),
                Err(error) => {
                    let (cause, ownership) = error.into_parts();
                    self.partial = Some(ownership);
                    return Err(Box::new(cause));
                }
            }
        }
        let Some(pending) = &mut self.constructor else {
            return Ok(false);
        };
        if pending.take_close_requested() {
            return Err("Installer browser requested closure".into());
        }
        let progress = pending.progress()?;
        if self.progress != Some(progress) {
            tracing::info!(
                operation = "installer_construction",
                ?progress,
                "Installer construction advanced"
            );
            self.progress = Some(progress);
        }
        if progress == ConstructionProgress::EnvironmentPrepared {
            if !self.observed {
                self.observer = Some(pending.private_profile_observer()?);
                if let Some(folder) = pending.user_data_folder()? {
                    use std::os::windows::ffi::OsStringExt;
                    lifetime.environment_observed(PathBuf::from(std::ffi::OsString::from_wide(
                        &folder.into_utf16(),
                    )))?;
                    self.observed = true;
                }
            } else if lifetime.environment_ready() {
                pending.authorize_environment()?;
            }
        }
        if progress != ConstructionProgress::Finished {
            return Ok(false);
        }
        let result = pending.take_result().ok_or("Installer controller result missing")?;
        self.constructor = None;
        match result {
            Ok(value) => {
                self.view = Some(value);
                Ok(true)
            }
            Err(error) => {
                let (cause, ownership) = error.into_parts();
                self.partial = Some(ownership);
                Err(Box::new(cause))
            }
        }
    }
    /// Cancel admission while retaining callbacks, native objects and the private profile grant.
    pub(super) fn cancel(&mut self) {
        if let Some(constructor) = &mut self.constructor {
            constructor.cancel();
        }
    }
    /// Release views first, then wait boundedly for this environment's actual collection exit.
    pub(super) fn retire(&mut self) -> Result<Option<BrowserRetirement>, Box<dyn Error>> {
        self.deadline.get_or_insert_with(|| Instant::now() + Duration::from_secs(15));
        if let Some(value) = &mut self.constructor {
            if value.checked_retire()? {
                self.constructor = None;
            }
            return Ok(None);
        }
        if let Some(value) = &mut self.partial {
            if value.checked_retire()? {
                self.partial = None;
            }
            return Ok(None);
        }
        if let Some(value) = &mut self.view {
            if value.checked_retire()? {
                self.view = None;
            }
            return Ok(None);
        }
        self.context = None;
        if !self.started {
            return Ok(Some(BrowserRetirement::NoEffects));
        }
        let Some(observer) = &mut self.observer else {
            return Ok(Some(BrowserRetirement::Unproven));
        };
        if observer.has_controller_identity()
            && !observer.released()
            && self.deadline.is_some_and(|deadline| Instant::now() < deadline)
        {
            return Ok(None);
        }
        let released = observer.finish()?;
        self.observer = None;
        Ok(Some(if released { BrowserRetirement::Released } else { BrowserRetirement::Unproven }))
    }
}
