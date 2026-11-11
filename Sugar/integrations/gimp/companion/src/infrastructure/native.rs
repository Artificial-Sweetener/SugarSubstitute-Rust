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
//! Grant private SDK access only to the local shell and its current renderer lifetime.

use std::{io::BufReader, sync::Mutex, time::Duration};
use sugarcubes_gimp_companion::native::{NativeLimits, NativeLink};
use tauri::Manager;

struct Viewer {
    link: NativeLink,
    generation: u32,
}
pub(super) struct NativeBridge(Mutex<Option<Viewer>>);

/// Admit only the compiled local asset origin, never the remote ComfyUI child webview.
pub(super) fn local(caller: &tauri::Webview) -> Result<(), String> {
    let url = caller
        .url()
        .map_err(|error| format!("Read native caller origin: {error}"))?;
    let local_origin = (url.scheme() == "tauri" && url.host_str() == Some("localhost"))
        || (matches!(url.scheme(), "http" | "https") && url.host_str() == Some("tauri.localhost"));
    if caller.label() != "companion" || !local_origin {
        return Err("Native GIMP commands belong to the local shell.".into());
    }
    Ok(())
}

/// Retire only the closing renderer; a delayed pagehide must not reset a newer renderer.
#[tauri::command]
pub(super) fn native_close(
    caller: tauri::Webview,
    state: tauri::State<'_, NativeBridge>,
    generation: u32,
) -> Result<(), String> {
    local(&caller)?;
    let mut viewer = state
        .0
        .lock()
        .map_err(|_| "The native viewer state is unavailable.")?;
    if let Some(viewer) = viewer
        .as_mut()
        .filter(|viewer| viewer.generation == generation)
    {
        viewer.link.reset()?;
        viewer.generation = viewer
            .generation
            .checked_add(1)
            .ok_or("The native viewer lifetime is exhausted.")?;
    }
    Ok(())
}

/// Start stdio only when the GIMP plug-in owns this process; preserve standalone shell diagnostics.
pub(super) fn install(
    app: &mut tauri::App,
    enabled: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let viewer = if enabled {
        let handle = app.handle().clone();
        let link = NativeLink::start(
            BufReader::new(std::io::stdin()),
            std::io::stdout(),
            NativeLimits::packaged()?,
            move |reason| {
                tracing::info!(operation="gimp_native_disconnect", cause=%reason, "The GIMP parent connection ended");
                handle.exit(0);
            },
        )?;
        Some(Viewer {
            link,
            generation: 0,
        })
    } else {
        None
    };
    app.manage(NativeBridge(Mutex::new(viewer)));
    Ok(())
}

/// Retire old renderer captures and return a fresh local-native generation.
#[tauri::command]
pub(super) fn native_open(
    caller: tauri::Webview,
    state: tauri::State<'_, NativeBridge>,
) -> Result<Option<u32>, String> {
    local(&caller)?;
    let mut viewer = state
        .0
        .lock()
        .map_err(|_| "The native viewer state is unavailable.")?;
    let Some(viewer) = viewer.as_mut() else {
        return Ok(None);
    };
    viewer.link.reset()?;
    viewer.generation = viewer
        .generation
        .checked_add(1)
        .ok_or("The native viewer lifetime is exhausted.")?;
    Ok(Some(viewer.generation))
}

/// Submit synchronously so cancellation can address a known native ticket before waiting.
#[tauri::command]
pub(super) fn native_submit(
    caller: tauri::Webview,
    state: tauri::State<'_, NativeBridge>,
    generation: u32,
    command: serde_json::Value,
) -> Result<u32, String> {
    local(&caller)?;
    let viewer = state
        .0
        .lock()
        .map_err(|_| "The native viewer state is unavailable.")?;
    let viewer = viewer
        .as_ref()
        .filter(|viewer| viewer.generation == generation)
        .ok_or("The native viewer lifetime is unavailable.")?;
    viewer.link.submit(command)
}

/// Wait off the UI thread without accepting another command or replaying a timed-out mutation.
#[tauri::command]
pub(super) async fn native_wait(
    caller: tauri::Webview,
    state: tauri::State<'_, NativeBridge>,
    generation: u32,
    id: u32,
) -> Result<serde_json::Value, String> {
    local(&caller)?;
    let link = {
        let viewer = state
            .0
            .lock()
            .map_err(|_| "The native viewer state is unavailable.")?;
        viewer
            .as_ref()
            .filter(|viewer| viewer.generation == generation)
            .ok_or("The native viewer lifetime is unavailable.")?
            .link
            .clone()
    };
    tauri::async_runtime::spawn_blocking(move || link.wait(id, Duration::from_secs(30)))
        .await
        .map_err(|error| format!("Wait for native GIMP: {error}"))?
}

/// Ignore a retired renderer while delivering current cancellation to the exact native request.
#[tauri::command]
pub(super) fn native_cancel(
    caller: tauri::Webview,
    state: tauri::State<'_, NativeBridge>,
    generation: u32,
    id: u32,
) -> Result<(), String> {
    local(&caller)?;
    let viewer = state
        .0
        .lock()
        .map_err(|_| "The native viewer state is unavailable.")?;
    if let Some(viewer) = viewer
        .as_ref()
        .filter(|viewer| viewer.generation == generation)
    {
        viewer.link.cancel(id)?;
    }
    Ok(())
}
