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
//! Construct the modeless WebView window with its SDK-supplied native parent already assigned.
use sugarcubes_gimp_companion::launch::NativeLaunch;
use tauri::WebviewWindowBuilder;
use windows::Win32::Foundation::HWND;

/// Read the private launch inputs once so window and native pipe admission share one decision.
pub(super) fn context() -> Result<NativeLaunch, Box<dyn std::error::Error>> {
    let optional = |name| match std::env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(error),
    };
    NativeLaunch::parse(
        optional("SUGARCUBES_GIMP_NATIVE")?.as_deref(),
        optional("SUGARCUBES_GIMP_OWNER")?.as_deref(),
        optional("SUGARCUBES_GIMP_IMAGE_NAME")?.as_deref(),
    )
    .map_err(Into::into)
}

/// Create an owned top-level window; retain native GIMP editing by keeping the owner enabled.
pub(super) fn install(
    app: &mut tauri::App,
    launch: &NativeLaunch,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == "companion")
        .ok_or("Missing companion window configuration.")?;
    let mut builder = WebviewWindowBuilder::from_config(app, config)?.title(launch.title());
    if let Some(owner) = launch.owner() {
        builder = builder.owner_raw(HWND(owner.get() as *mut std::ffi::c_void));
    } else if launch.native() {
        tracing::info!(
            operation = "gimp_window_association",
            "The GIMP SDK supplied no display window; the companion window is independent"
        );
    }
    builder.build()?;
    Ok(())
}
