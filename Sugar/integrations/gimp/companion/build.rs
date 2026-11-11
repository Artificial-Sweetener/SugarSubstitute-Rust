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
//! Generate Tauri's native manifest only for the Windows prototype host.

#[cfg(all(windows, feature = "desktop"))]
fn main() {
    let attributes =
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "connect_comfy",
            "native_open",
            "native_submit",
            "native_wait",
            "native_cancel",
            "native_close",
            "panel_open",
            "panel_attach",
            "panel_send",
            "panel_visible",
        ]));
    if let Err(error) = tauri_build::try_build(attributes) {
        eprintln!("Build GIMP companion manifest: {error:#}");
        std::process::exit(1);
    }
}

#[cfg(not(all(windows, feature = "desktop")))]
fn main() {}
