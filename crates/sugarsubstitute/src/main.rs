// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Enter the desktop library on the operating system's main thread.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
fn main() -> std::process::ExitCode {
    sugarsubstitute::run()
}
