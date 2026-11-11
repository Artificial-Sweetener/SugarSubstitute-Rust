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
//! Own Tauri windows, native connection admission and bounded background readiness probes.

mod native;
mod panel;
mod probe;
mod window;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use sugarcubes_gimp_companion::connection::{ComfyEndpoint, ConnectionState};
use tauri::{LogicalPosition, Manager, WebviewUrl, webview::WebviewBuilder};

struct NativeConnection {
    endpoint: ComfyEndpoint,
    probing: Arc<AtomicBool>,
}

/// Verify WebView2 through Tauri's public adapter before creating a native window or starting SDK IO.
pub(super) fn check_runtime() -> Result<(), Box<dyn std::error::Error>> {
    let report = match tauri::webview_version() {
        Ok(version) => serde_json::json!({"status": "ready", "version": version}),
        Err(error) => serde_json::json!({
            "status": "unavailable",
            "message": "SugarCubes requires Microsoft Edge WebView2 Runtime. Install it, then reopen SugarCubes.",
            "detail": error.to_string(),
        }),
    };
    serde_json::to_writer(std::io::stdout(), &report)?;
    if report["status"] == "unavailable" {
        return Err("The companion runtime is unavailable.".into());
    }
    Ok(())
}

/// Keep a running native probe admitted even if its invoking renderer disappears.
struct ProbeLease(Arc<AtomicBool>);
impl Drop for ProbeLease {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// Connect the local shell without granting remote ComfyUI content control of host lifecycle.
#[tauri::command]
async fn connect_comfy(
    app: tauri::AppHandle,
    caller: tauri::Webview,
    state: tauri::State<'_, NativeConnection>,
    session: String,
) -> Result<ConnectionState, String> {
    native::local(&caller)?;
    let route = panel::route(&app, &session)?;
    let endpoint = state.endpoint.clone();
    if state
        .probing
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("A connection attempt is already running.".into());
    }
    let lease = ProbeLease(state.probing.clone());
    let probe_endpoint = endpoint.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _lease = lease;
        probe::ready(&probe_endpoint)
    })
    .await
    .map_err(|error| format!("Connection worker failed: {error}"))?;
    if let Err(error) = result {
        tracing::info!(operation="connect_comfy", cause=%error, "ComfyUI is unavailable");
        return Ok(ConnectionState {
            phase: "disconnected",
            address: endpoint.address().into(),
            message: "Start the configured ComfyUI server, then retry.".into(),
        });
    }
    // Revalidate after the readiness probe; a shell reload may have replaced this session.
    panel::route(&app, &session)?;
    if let Some(guest) = app.get_webview("comfy") {
        if !panel::guest_attached(&app, &session) {
            guest
                .navigate(route.url().clone())
                .map_err(|error| error.to_string())?;
        }
    } else {
        let window = app
            .get_window("companion")
            .ok_or("The companion window has closed.")?;
        let size = window.inner_size().map_err(|error| error.to_string())?;
        let scale = window.scale_factor().map_err(|error| error.to_string())?;
        let navigation_app = app.clone();
        let builder = WebviewBuilder::new("comfy", WebviewUrl::External(route.url().clone()))
            .initialization_script(include_str!(env!("SUGARCUBES_GIMP_GUEST")))
            .auto_resize()
            .on_navigation(move |destination| {
                panel::accepts_navigation(&navigation_app, destination)
            });
        let guest = window
            .add_child(
                builder,
                LogicalPosition::new(0.0, 0.0),
                size.to_logical::<f64>(scale),
            )
            .map_err(|error| error.to_string())?;
        guest.hide().map_err(|error| error.to_string())?;
    }
    Ok(ConnectionState {
        phase: "connecting",
        address: endpoint.address().into(),
        message: "Waiting for SugarCubes…".into(),
    })
}

/// Initialize one host lifetime without starting ComfyUI or installing any editor components.
pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .try_init()
        .map_err(|error| format!("Initialize companion diagnostics: {error}"))?;
    let address = match std::env::var("SUGARCUBES_COMFY_URL") {
        Ok(address) => address,
        Err(std::env::VarError::NotPresent) => "http://127.0.0.1:8188/".into(),
        Err(error) => return Err(error.into()),
    };
    let endpoint = ComfyEndpoint::parse(&address)?;
    let launch = window::context()?;
    tauri::Builder::default()
        .manage(NativeConnection {
            endpoint: endpoint.clone(),
            probing: Arc::new(AtomicBool::new(false)),
        })
        .setup(move |app| {
            native::install(app, launch.native())?;
            panel::install(app, endpoint)?;
            window::install(app, &launch)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            connect_comfy,
            native::native_open,
            native::native_submit,
            native::native_wait,
            native::native_cancel,
            native::native_close,
            panel::panel_open,
            panel::panel_attach,
            panel::panel_send,
            panel::panel_visible
        ])
        .run(tauri::generate_context!())?;
    Ok(())
}
