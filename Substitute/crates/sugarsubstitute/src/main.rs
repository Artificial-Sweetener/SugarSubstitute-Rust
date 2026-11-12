// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Enter the desktop library on the operating system's main thread.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
fn main() -> std::process::ExitCode {
    #[cfg(all(windows, feature = "windows-native-host"))]
    #[allow(unsafe_code)]
    // SAFETY: this is the initial binary entry. No application resources, diagnostics,
    // execution workers or local handle owners have been initialized. Capture occurs once.
    let selection = unsafe {
        sugarsubstitute_supervision::setup_storage::capture_launch(std::env::args_os().skip(1))
    };
    sugarsubstitute::run(
        #[cfg(all(windows, feature = "windows-native-host"))]
        selection,
    )
}
