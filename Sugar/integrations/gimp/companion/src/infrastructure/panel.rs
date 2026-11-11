//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
//! Relay existing public protocols between two explicitly scoped WebViews.

use serde_json::Value;
use std::sync::Mutex;
use sugarcubes_gimp_companion::{
    connection::ComfyEndpoint, native::NativeLimits, panel::PanelRoute,
};
use tauri::{Manager, ipc::Channel};

struct Session {
    route: PanelRoute,
    host: Channel<Value>,
    guest: Option<Channel<Value>>,
}
pub(super) struct PanelBridge {
    endpoint: ComfyEndpoint,
    maximum: usize,
    session: Mutex<Option<Session>>,
}

/// Scope the remote page to two relay commands and its exact configured loopback origin.
pub(super) fn install(
    app: &mut tauri::App,
    endpoint: ComfyEndpoint,
) -> Result<(), Box<dyn std::error::Error>> {
    app.add_capability(
        tauri::ipc::CapabilityBuilder::new("comfy-public-panel")
            .webview("comfy")
            .local(false)
            .remote(format!("{}*", endpoint.address()))
            .permission("allow-panel-attach")
            .permission("allow-panel-send"),
    )?;
    app.manage(PanelBridge {
        endpoint,
        maximum: NativeLimits::packaged()?.frame_bytes,
        session: Mutex::new(None),
    });
    Ok(())
}

/// Select the existing shared public session before opening the remote interface.
#[tauri::command]
pub(super) fn panel_open(
    caller: tauri::Webview,
    state: tauri::State<'_, PanelBridge>,
    session: String,
    channel: Channel<Value>,
) -> Result<(), String> {
    super::native::local(&caller)?;
    let route = PanelRoute::new(&state.endpoint, session)?;
    *state
        .session
        .lock()
        .map_err(|_| "The panel state is unavailable.")? = Some(Session {
        route,
        host: channel,
        guest: None,
    });
    Ok(())
}

/// Resolve only the currently admitted route when the local shell asks to navigate.
pub(super) fn route(app: &tauri::AppHandle, session: &str) -> Result<PanelRoute, String> {
    let state = app.state::<PanelBridge>();
    let session_guard = state
        .session
        .lock()
        .map_err(|_| "The panel state is unavailable.")?;
    session_guard
        .as_ref()
        .filter(|active| active.route.session() == session)
        .map(|active| active.route.clone())
        .ok_or_else(|| "The panel session is stale.".into())
}

/// Apply the current route to each top-level navigation, including retries that reuse the WebView.
pub(super) fn accepts_navigation(app: &tauri::AppHandle, url: &url::Url) -> bool {
    let state = app.state::<PanelBridge>();
    state.session.lock().ok().is_some_and(|guard| {
        guard
            .as_ref()
            .is_some_and(|active| active.route.accepts_navigation(url))
    })
}

/// Verify remote origin, WebView role and navigation identity before accepting a callback.
fn guest(caller: &tauri::Webview, active: &Session, session: &str) -> Result<(), String> {
    let url = caller.url().map_err(|error| error.to_string())?;
    if caller.label() != "comfy"
        || active.route.session() != session
        || !active.route.accepts_origin(&url)
    {
        return Err("The public panel caller is stale or untrusted.".into());
    }
    Ok(())
}

/// Preserve the loaded SPA when its history URL no longer contains the original query parameters.
pub(super) fn guest_attached(app: &tauri::AppHandle, session: &str) -> bool {
    let state = app.state::<PanelBridge>();
    state.session.lock().ok().is_some_and(|guard| {
        guard
            .as_ref()
            .is_some_and(|active| active.route.session() == session && active.guest.is_some())
    })
}

/// Bind the remote renderer's receive callback without exposing private native commands.
#[tauri::command]
pub(super) fn panel_attach(
    caller: tauri::Webview,
    state: tauri::State<'_, PanelBridge>,
    session: String,
    channel: Channel<Value>,
) -> Result<(), String> {
    let mut guard = state
        .session
        .lock()
        .map_err(|_| "The panel state is unavailable.")?;
    let active = guard.as_mut().ok_or("The panel session is unavailable.")?;
    guest(&caller, active, &session)?;
    active.guest = Some(channel);
    Ok(())
}

/// Forward an opaque public envelope after checking both the caller and target lifetimes.
#[tauri::command]
pub(super) fn panel_send(
    caller: tauri::Webview,
    state: tauri::State<'_, PanelBridge>,
    session: String,
    message: Value,
) -> Result<(), String> {
    let guard = state
        .session
        .lock()
        .map_err(|_| "The panel state is unavailable.")?;
    let active = guard.as_ref().ok_or("The panel session is unavailable.")?;
    if session != active.route.session() || !active.route.accepts_message(&message, state.maximum) {
        return Err("The public panel envelope is stale or exceeds its transport budget.".into());
    }
    let target = if caller.label() == "companion" {
        super::native::local(&caller)?;
        active
            .guest
            .as_ref()
            .ok_or("The ComfyUI panel is not ready.")?
    } else {
        guest(&caller, active, &session)?;
        &active.host
    };
    target
        .send(message)
        .map_err(|error| format!("Deliver public panel envelope: {error}"))
}

/// Reveal local connection feedback without destroying the loaded workflow on backend disconnect.
#[tauri::command]
pub(super) fn panel_visible(
    app: tauri::AppHandle,
    caller: tauri::Webview,
    session: String,
    visible: bool,
) -> Result<(), String> {
    super::native::local(&caller)?;
    route(&app, &session)?;
    if let Some(guest) = app.get_webview("comfy") {
        if visible { guest.show() } else { guest.hide() }.map_err(|error| error.to_string())?;
    }
    Ok(())
}
