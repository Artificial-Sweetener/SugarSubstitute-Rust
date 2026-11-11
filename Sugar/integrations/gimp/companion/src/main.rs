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
//! Launch the Windows prototype host while keeping portable connection rules in the library.
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod infrastructure;

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().skip(1).eq(["--check-runtime"]) {
        return infrastructure::check_runtime();
    }
    infrastructure::run()
}

#[cfg(not(windows))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err("The GIMP companion prototype currently targets Windows.".into())
}
