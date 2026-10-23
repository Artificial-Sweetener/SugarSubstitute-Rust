// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Native branded setup entry point.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(all(windows, feature = "windows-native-host"))]
    {
        sugarsubstitute::setup::run(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/installer-distribution.json"
        )))
    }
    #[cfg(not(all(windows, feature = "windows-native-host")))]
    {
        Err("The native installer adapter is not available on this platform".into())
    }
}
