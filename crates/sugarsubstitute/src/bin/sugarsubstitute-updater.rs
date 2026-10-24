// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Independently executable native update worker; owns no application window or run lease.
#![cfg_attr(windows, windows_subsystem = "windows")]
fn main() -> std::process::ExitCode {
    #[cfg(all(windows, feature = "windows-native-host"))]
    {
        #[allow(unsafe_code)]
        // SAFETY: initial binary entry precedes diagnostics, workers and all local handle owners.
        // Capture runs exactly once; no other code can adopt or reuse an inherited handle yet.
        let probe = unsafe {
            sugarsubstitute_supervision::setup_storage::capture_updater_probe(
                std::env::args_os().skip(1),
            )
        };
        sugarsubstitute::updates::entry(probe)
    }
    #[cfg(not(all(windows, feature = "windows-native-host")))]
    {
        sugarsubstitute::unavailable_native_entry(sugarsubstitute::NativeEntry::Updater)
    }
}
