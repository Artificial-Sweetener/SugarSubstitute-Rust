// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native branded setup entry point.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() -> std::process::ExitCode {
    #[cfg(all(windows, feature = "windows-native-host"))]
    {
        match sugarsubstitute::setup::run(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/installer-distribution.json"
        ))) {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(_retained) => std::process::ExitCode::FAILURE,
        }
    }
    #[cfg(not(all(windows, feature = "windows-native-host")))]
    {
        sugarsubstitute::unavailable_native_entry(sugarsubstitute::NativeEntry::Installer)
    }
}
