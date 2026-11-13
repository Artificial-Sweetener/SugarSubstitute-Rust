// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route disabled native executable entries through the supported diagnostic lifetime.
use std::{error::Error, io, process::ExitCode};
/// Identify a native executable whose adapter is unavailable in this build.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeEntry {
    /// Branded installer executable.
    Installer,
    /// Stable installed launcher executable.
    Launcher,
    /// Independent updater executable.
    Updater,
    /// Standalone native canvas image viewer.
    Viewer,
}
impl NativeEntry {
    /// Select a closed diagnostic code without accepting caller-provided text.
    fn name(self) -> &'static str {
        match self {
            Self::Installer => "installer",
            Self::Launcher => "launcher",
            Self::Updater => "updater",
            Self::Viewer => "viewer",
        }
    }
}
/// Report one unavailable native entry and observe diagnostic teardown before returning failure.
/// Product copy uses the canonical localization fallback. Logging workers retire before process status returns.
pub fn unavailable_native_entry(entry: NativeEntry) -> ExitCode {
    let result: Result<(), Box<dyn Error>> = crate::diagnostics::Diagnostics::run(
        crate::diagnostics::RecordsBase::RunningImage,
        |_health| {
            let message =
                sugarsubstitute_contracts::localization::messages("en").report.startup_failed;
            tracing::error!(
                operation = "native_adapter_unavailable",
                adapter = entry.name(),
                "{message}"
            );
            Err(io::Error::new(io::ErrorKind::Unsupported, message).into())
        },
    );
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(_retained) => ExitCode::FAILURE,
    }
}
