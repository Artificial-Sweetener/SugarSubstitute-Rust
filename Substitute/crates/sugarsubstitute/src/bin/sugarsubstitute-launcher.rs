// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Stable native installation entry point.
#![cfg_attr(windows, windows_subsystem = "windows")]
fn main() -> std::process::ExitCode {
    #[cfg(all(windows, feature = "windows-native-host"))]
    {
        sugarsubstitute::stable_launcher::run()
    }
    #[cfg(not(all(windows, feature = "windows-native-host")))]
    {
        sugarsubstitute::unavailable_native_entry(sugarsubstitute::NativeEntry::Launcher)
    }
}
