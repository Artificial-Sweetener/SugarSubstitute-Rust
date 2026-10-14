// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Guard actual top navigation and deny unowned browser device/download effects.

use super::*;
use std::cell::Cell;
use sugarsubstitute_contracts::configuration::LoopbackOrigin;
#[cfg(feature = "k1-native-qualification")]
use webview2_com::ContentLoadingEventHandler;
use webview2_com::{
    DownloadStartingEventHandler, NavigationCompletedEventHandler, NavigationStartingEventHandler,
    PermissionRequestedEventHandler, ProcessFailedEventHandler,
};

const EVENT_COUNT: usize = if cfg!(feature = "k1-native-qualification") { 6 } else { 5 };

struct State {
    origin: LoopbackOrigin,
    active: Cell<bool>,
    navigation: Cell<Option<u64>>,
    #[cfg(feature = "k1-native-qualification")]
    content: Cell<Option<(u64, bool)>>,
    #[cfg(feature = "k1-native-qualification")]
    completed: Cell<Option<u64>>,
    #[cfg(feature = "k1-native-qualification")]
    content_failure: Cell<Option<i32>>,
}
pub(super) struct Navigation {
    tokens: [Option<i64>; EVENT_COUNT],
    uncertain: bool,
    installed: usize,
    state: Rc<State>,
}
impl Navigation {
    pub(super) fn new(origin: LoopbackOrigin) -> Self {
        Self {
            tokens: [None; EVENT_COUNT],
            uncertain: false,
            installed: 0,
            state: Rc::new(State {
                origin,
                active: Cell::new(false),
                navigation: Cell::new(None),
                #[cfg(feature = "k1-native-qualification")]
                content: Cell::new(None),
                #[cfg(feature = "k1-native-qualification")]
                completed: Cell::new(None),
                #[cfg(feature = "k1-native-qualification")]
                content_failure: Cell::new(None),
            }),
        }
    }
    pub(super) fn begin(&self) -> Result<(), DesktopError> {
        if self.installed != self.tokens.len() || self.uncertain || self.state.active.get() {
            return Err(DesktopError::message(
                "begin_native_navigation",
                "navigation receivers are not current",
            ));
        }
        self.state.active.set(true);
        Ok(())
    }
    pub(super) fn install(
        &mut self,
        core: &ICoreWebView2,
        events: &Rc<Events>,
    ) -> Result<bool, DesktopError> {
        if self.installed == self.tokens.len() {
            return Ok(true);
        }
        let captured = events.clone();
        let state = self.state.clone();
        let mut token = 0;
        // SAFETY: Each callback retains only UI-affine metadata/storage; install token is
        // captured before status interpretation, and native getters precede queue borrowing.
        let result = unsafe {
            match self.installed {
                0 => core.add_NavigationStarting(
                    &NavigationStartingEventHandler::create(Box::new(move |_, args| {
                        let _callback = captured.callback();
                        let mut pointer = PWSTR::null();
                        let mut id = 0;
                        let mut admitted = false;
                        if let Some(args) = args {
                            let status = args.Uri(&mut pointer);
                            let returned = NativeString(pointer);
                            let result = status.and_then(|()| args.NavigationId(&mut id));
                            if let Err(cause) = result {
                                captured.fail(DesktopError::new("native_navigation_input", cause));
                            } else if returned.0.is_null() {
                                captured.fail(DesktopError::new(
                                    "native_navigation_input",
                                    windows::core::Error::from(E_POINTER),
                                ));
                            } else {
                                captured.record(Kind::NavigationStarting, |text| {
                                    crate::main_events::copy_utf16(text, |index| {
                                        returned.0.as_ptr().add(index).read()
                                    })?;
                                    admitted = if !state.active.get() {
                                        text == "about:blank"
                                    } else {
                                        id != 0
                                            && crate::main_navigation::admits_url(
                                                state.origin,
                                                text,
                                            )
                                            && state.navigation.get().is_none_or(|old| old == id)
                                    };
                                    if admitted && state.active.get() {
                                        state.navigation.set(Some(id));
                                    }
                                    if !admitted {
                                        return Err(DesktopError::message(
                                            "native_navigation_input",
                                            "unadmitted native navigation",
                                        ));
                                    }
                                    text.clear();
                                    Ok(())
                                });
                            }
                            if let Err(cause) = args.SetCancel(!admitted) {
                                captured.fail(DesktopError::new("cancel_native_navigation", cause));
                            }
                        } else {
                            captured.fail(DesktopError::new(
                                "native_navigation_input",
                                windows::core::Error::from(E_POINTER),
                            ));
                        }
                        Ok(())
                    })),
                    &mut token,
                ),
                1 => core.add_NavigationCompleted(
                    &NavigationCompletedEventHandler::create(Box::new(move |_, args| {
                        let _callback = captured.callback();
                        if let Some(args) = args {
                            let mut id = 0;
                            let mut success = windows::core::BOOL::default();
                            let result = args
                                .NavigationId(&mut id)
                                .and_then(|()| args.IsSuccess(&mut success));
                            if let Err(cause) = result {
                                captured
                                    .fail(DesktopError::new("native_navigation_completion", cause));
                            } else if state.active.get() && state.navigation.get() == Some(id) {
                                #[cfg(feature = "k1-native-qualification")]
                                state.completed.set(success.as_bool().then_some(id));
                                captured.record(
                                    Kind::Navigation { succeeded: success.as_bool() },
                                    |_| Ok(()),
                                );
                            }
                        } else {
                            captured.fail(DesktopError::new(
                                "native_navigation_completion",
                                windows::core::Error::from(E_POINTER),
                            ));
                        }
                        Ok(())
                    })),
                    &mut token,
                ),
                2 => core.add_ProcessFailed(
                    &ProcessFailedEventHandler::create(Box::new(move |_, args| {
                        let _callback = captured.callback();
                        if let Some(args) = args {
                            let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
                            match args.ProcessFailedKind(&mut kind) {
                                Ok(()) => captured
                                    .record(Kind::ProcessFailed { kind: kind.0 }, |_| Ok(())),
                                Err(cause) => captured
                                    .fail(DesktopError::new("native_browser_failure", cause)),
                            }
                        } else {
                            captured.fail(DesktopError::new(
                                "native_browser_failure",
                                windows::core::Error::from(E_POINTER),
                            ));
                        }
                        Ok(())
                    })),
                    &mut token,
                ),
                3 => core.add_PermissionRequested(
                    &PermissionRequestedEventHandler::create(Box::new(move |_, args| {
                        let _callback = captured.callback();
                        let result = match args {
                            Some(args) => args.SetState(COREWEBVIEW2_PERMISSION_STATE_DENY),
                            None => Err(windows::core::Error::from(E_POINTER)),
                        };
                        if let Err(cause) = result {
                            captured
                                .fail(DesktopError::new("deny_unowned_device_permission", cause));
                        }
                        Ok(())
                    })),
                    &mut token,
                ),
                4 => core.cast::<ICoreWebView2_4>().and_then(|core| {
                    core.add_DownloadStarting(
                        &DownloadStartingEventHandler::create(Box::new(move |_, args| {
                            let _callback = captured.callback();
                            let result = match args {
                                Some(args) => args.SetCancel(true),
                                None => Err(windows::core::Error::from(E_POINTER)),
                            };
                            if let Err(cause) = result {
                                captured.fail(DesktopError::new("deny_unowned_download", cause));
                            }
                            Ok(())
                        })),
                        &mut token,
                    )
                }),
                #[cfg(feature = "k1-native-qualification")]
                5 => core.add_ContentLoading(
                    &ContentLoadingEventHandler::create(Box::new(move |_, args| {
                        let _callback = captured.callback();
                        let mut id = 0;
                        let mut error_page = windows::core::BOOL::default();
                        let observed = match args {
                            Some(args) => args
                                .NavigationId(&mut id)
                                .and_then(|()| args.IsErrorPage(&mut error_page)),
                            None => Err(windows::core::Error::from(E_POINTER)),
                        };
                        match observed {
                            Ok(()) if state.active.get() && state.navigation.get() == Some(id) => {
                                state.content.set(Some((id, error_page.as_bool())))
                            }
                            Ok(()) => {}
                            Err(cause) if state.content_failure.get().is_none() => {
                                state.content_failure.set(Some(cause.code().0))
                            }
                            Err(_) => {}
                        }
                        Ok(())
                    })),
                    &mut token,
                ),
                _ => {
                    return Err(DesktopError::message(
                        "install_main_browser_events",
                        "invalid event index",
                    ));
                }
            }
        };
        if result.is_ok() || token != 0 {
            self.tokens[self.installed] = Some(token);
        } else {
            self.uncertain = true;
        }
        result.map_err(|cause| DesktopError::new("install_main_browser_events", cause))?;
        self.installed += 1;
        Ok(false)
    }

    /// Bind qualification to actual current top content, never an outgoing document capture.
    #[cfg(feature = "k1-native-qualification")]
    pub(super) fn preview_navigation(&self) -> Result<u64, DesktopError> {
        if let Some(status) = self.state.content_failure.get() {
            return Err(DesktopError::new(
                "observe_preview_content",
                windows::core::Error::from(windows::core::HRESULT(status)),
            ));
        }
        match self.state.content.get() {
            Some((id, false))
                if self.state.active.get()
                    && self.state.navigation.get() == Some(id)
                    && self.state.completed.get() == Some(id) =>
            {
                Ok(id)
            }
            _ => Err(DesktopError::message(
                "observe_preview_content",
                "current native content is not eligible",
            )),
        }
    }
    pub(super) fn retire_step(&mut self, core: &ICoreWebView2) -> Result<bool, DesktopError> {
        if self.uncertain {
            return Ok(false);
        }
        let Some(index) = self.tokens.iter().rposition(Option::is_some) else { return Ok(true) };
        let token = self.tokens[index].ok_or_else(|| {
            DesktopError::message("remove_main_browser_events", "missing event token")
        })?;
        // SAFETY: Every exact token belongs to this retained native core and current UI thread.
        unsafe {
            match index {
                0 => core.remove_NavigationStarting(token),
                1 => core.remove_NavigationCompleted(token),
                2 => core.remove_ProcessFailed(token),
                3 => core.remove_PermissionRequested(token),
                4 => core
                    .cast::<ICoreWebView2_4>()
                    .and_then(|core| core.remove_DownloadStarting(token)),
                #[cfg(feature = "k1-native-qualification")]
                5 => core.remove_ContentLoading(token),
                _ => {
                    return Err(DesktopError::message(
                        "remove_main_browser_events",
                        "invalid token index",
                    ));
                }
            }
        }
        .map_err(|cause| DesktopError::new("remove_main_browser_events", cause))?;
        self.tokens[index] = None;
        Ok(false)
    }
}
