// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Stable native installation entry point.
#![cfg_attr(windows, windows_subsystem = "windows")]
fn main() -> std::process::ExitCode {
    #[cfg(all(windows, feature = "windows-native-host"))]
    {
        #[allow(unsafe_code)]
        // SAFETY: initial binary entry precedes diagnostics, workers and all local handle owners.
        // Capture runs exactly once; no other code can adopt or reuse the inherited event yet.
        let engine = unsafe {
            sugarsubstitute_supervision::application_launch::capture_engine_launch(
                std::env::args_os().skip(1),
            )
        };
        sugarsubstitute::stable_launcher::run(engine)
    }
    #[cfg(not(all(windows, feature = "windows-native-host")))]
    {
        sugarsubstitute::unavailable_native_entry(sugarsubstitute::NativeEntry::Launcher)
    }
}
