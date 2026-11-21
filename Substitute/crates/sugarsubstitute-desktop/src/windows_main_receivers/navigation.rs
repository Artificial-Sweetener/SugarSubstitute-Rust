// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Guard actual top navigation, route browser downloads through their retained owner, and keep
//! new windows out of the embedded browser.

use super::*;
use std::cell::Cell;
use sugarsubstitute_contracts::configuration::LoopbackOrigin;
#[cfg(feature = "k1-native-qualification")]
use webview2_com::ContentLoadingEventHandler;
use webview2_com::{
    DownloadStartingEventHandler, NavigationCompletedEventHandler, NavigationStartingEventHandler,
    NewWindowRequestedEventHandler, PermissionRequestedEventHandler, ProcessFailedEventHandler,
};

const EVENT_COUNT: usize = if cfg!(feature = "k1-native-qualification") { 7 } else { 6 };

struct State {
    origin: LoopbackOrigin,
    active: Cell<bool>,
    revealed: Cell<bool>,
    reload_requested: Cell<bool>,
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
                revealed: Cell::new(false),
                reload_requested: Cell::new(false),
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
    pub(super) fn revealed(&self) {
        self.state.revealed.set(true);
    }
    pub(super) fn install(
        &mut self,
        core: &ICoreWebView2,
        events: &Rc<Events>,
        downloads: &Rc<super::downloads::Owner>,
    ) -> Result<bool, DesktopError> {
        if self.installed == self.tokens.len() {
            return Ok(true);
        }
        let captured = events.clone();
        let state = self.state.clone();
        let downloads = downloads.clone();
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
                        let mut request_reload = false;
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
                                    if !admitted
                                        && state.active.get()
                                        && state.revealed.get()
                                        && id != 0
                                        && crate::main_navigation::admits_url(state.origin, text)
                                    {
                                        request_reload = !state.reload_requested.replace(true);
                                        text.clear();
                                        return Ok(());
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
                            if request_reload {
                                captured.record(Kind::ReloadRequested, |_| Ok(()));
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
                // A clipboard read the person asked for on the application's own page is allowed,
                // so the prompt menu's Paste works; every other permission stays denied. No answer
                // is saved in the profile, and a refusal an older build saved is cleared first.
                3 => {
                    clear_saved_clipboard_refusal(core, state.origin);
                    core.add_PermissionRequested(
                        &PermissionRequestedEventHandler::create(Box::new(move |_, args| {
                            let _callback = captured.callback();
                            let result = match args {
                                Some(args) => answer_permission(&args, state.origin),
                                None => Err(windows::core::Error::from(E_POINTER)),
                            };
                            if let Err(cause) = result {
                                captured.fail(DesktopError::new(
                                    "deny_unowned_device_permission",
                                    cause,
                                ));
                            }
                            Ok(())
                        })),
                        &mut token,
                    )
                }
                4 => core.cast::<ICoreWebView2_4>().and_then(|core| {
                    core.add_DownloadStarting(
                        &DownloadStartingEventHandler::create(Box::new(move |sender, args| {
                            let _callback = captured.callback();
                            if let Err(cause) = downloads.starting(
                                sender.as_ref(),
                                args.as_ref(),
                                state.revealed.get(),
                                &captured,
                            ) {
                                captured.fail(cause);
                            }
                            Ok(())
                        })),
                        &mut token,
                    )
                }),
                // A new window never opens inside the app; its link is queued for the browser,
                // which the evidence owner asks the shell to open between callbacks.
                5 => core.add_NewWindowRequested(
                    &NewWindowRequestedEventHandler::create(Box::new(move |_, args| {
                        let _callback = captured.callback();
                        let Some(args) = args else {
                            captured.fail(DesktopError::new(
                                "native_new_window_input",
                                windows::core::Error::from(E_POINTER),
                            ));
                            return Ok(());
                        };
                        if let Err(cause) =
                            args.SetHandled(!crate::windows_open_link::EMBEDDED_WINDOWS)
                        {
                            captured.fail(DesktopError::new("cancel_native_new_window", cause));
                            return Ok(());
                        }
                        let mut pointer = PWSTR::null();
                        let status = args.Uri(&mut pointer);
                        let returned = NativeString(pointer);
                        match status {
                            Ok(()) if !returned.0.is_null() => {
                                captured.record(Kind::OpenLink, |text| {
                                    crate::main_events::copy_utf16(text, |index| {
                                        returned.0.as_ptr().add(index).read()
                                    })
                                });
                            }
                            Ok(()) => tracing::warn!(
                                operation = "native_new_window_input",
                                "A new-window request named no link"
                            ),
                            Err(cause) => tracing::warn!(operation = "native_new_window_input",
                                %cause, "A new-window request's link could not be read"),
                        }
                        Ok(())
                    })),
                    &mut token,
                ),
                #[cfg(feature = "k1-native-qualification")]
                6 => core.add_ContentLoading(
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
        let Some(index) = self.tokens.iter().rposition(Option::is_some) else {
            return Ok(true);
        };
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
                5 => core.remove_NewWindowRequested(token),
                #[cfg(feature = "k1-native-qualification")]
                6 => core.remove_ContentLoading(token),
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

/// Answer one permission request by the application's permission rule, saving nothing.
fn answer_permission(
    args: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2PermissionRequestedEventArgs,
    origin: LoopbackOrigin,
) -> windows::core::Result<()> {
    use crate::main_navigation::{PermissionRequest, admits_permission};
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_PERMISSION_KIND, COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ,
        COREWEBVIEW2_PERMISSION_STATE_ALLOW, ICoreWebView2PermissionRequestedEventArgs3,
    };
    let mut kind = COREWEBVIEW2_PERMISSION_KIND::default();
    let mut pointer = PWSTR::null();
    let mut user_initiated = windows::core::BOOL::default();
    // SAFETY: WebView2 owns `args` for this callback; each getter writes only its out value.
    unsafe {
        args.PermissionKind(&mut kind)?;
        args.Uri(&mut pointer)?;
        args.IsUserInitiated(&mut user_initiated)?;
    }
    let returned = NativeString(pointer);
    // SAFETY: WebView2 returned a NUL-terminated string that `returned` frees afterwards.
    let uri = unsafe { returned.0.to_string() }.unwrap_or_default();
    let request = if kind == COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ {
        PermissionRequest::ClipboardRead
    } else {
        PermissionRequest::Other
    };
    let allowed = admits_permission(origin, request, &uri, user_initiated.as_bool());
    if let Ok(saving) = args.cast::<ICoreWebView2PermissionRequestedEventArgs3>() {
        // SAFETY: A setter on the same live arguments.
        unsafe { saving.SetSavesInProfile(false)? };
    }
    tracing::info!(
        operation = "browser_permission_answered",
        clipboard = request == PermissionRequest::ClipboardRead,
        allowed,
        "Answered a page permission request"
    );
    let answer = if allowed {
        COREWEBVIEW2_PERMISSION_STATE_ALLOW
    } else {
        COREWEBVIEW2_PERMISSION_STATE_DENY
    };
    // SAFETY: A setter on the same live arguments.
    unsafe { args.SetState(answer) }
}

/// Clear a clipboard-read refusal an older build let WebView2 save in the profile, so the page
/// is asked again. Failure leaves the saved answer and is logged; nothing else depends on it.
fn clear_saved_clipboard_refusal(core: &ICoreWebView2, origin: LoopbackOrigin) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ, COREWEBVIEW2_PERMISSION_STATE_DEFAULT,
        ICoreWebView2_13, ICoreWebView2Profile4,
    };
    let page = crate::main_navigation::root_url(origin);
    let wide: Vec<u16> = page.trim_end_matches('/').encode_utf16().chain(Some(0)).collect();
    let result = core
        .cast::<ICoreWebView2_13>()
        // SAFETY: A getter on the live core.
        .and_then(|core| unsafe { core.Profile() })
        .and_then(|profile| profile.cast::<ICoreWebView2Profile4>())
        // SAFETY: `wide` is NUL-terminated and outlives the call; WebView2 copies the origin.
        .and_then(|profile| unsafe {
            profile.SetPermissionState(
                COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ,
                windows::core::PCWSTR(wide.as_ptr()),
                COREWEBVIEW2_PERMISSION_STATE_DEFAULT,
                None,
            )
        });
    if let Err(cause) = result {
        tracing::warn!(operation = "clear_saved_clipboard_refusal", cause = %cause,
            "A saved clipboard refusal could not be cleared");
    }
}
