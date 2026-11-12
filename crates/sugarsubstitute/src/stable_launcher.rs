// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Stable installed entry point; the selected application owns the only visible startup window.
mod lifetime;
use std::{
    error::Error,
    ffi::OsString,
    path::{Path, PathBuf},
    process::ExitCode,
};

/// Run the accepted engine's startup role once, authenticating its actual image before selection.
/// Called on the native main thread under the updater entry's diagnostics lifetime.
pub(crate) fn activate(
    source: std::sync::Arc<
        sugarsubstitute_supervision::application_launch::broker::ActivationSender,
    >,
    executable: PathBuf,
    repository: [u8; 32],
) -> Result<(), Box<dyn Error>> {
    sugarsubstitute_desktop::run_headless(&mut lifetime::Launcher::activation(
        source, executable, repository,
    ))?;
    Ok(())
}

/// Run beside the installation record without acquiring the application run lock.
/// Must be called once on the native main thread. Selected application startup is independent
/// after creation; this process retires its own worker scopes and inherited-handle duplicates.
pub fn run() -> ExitCode {
    let result = crate::diagnostics::Diagnostics::run(
        crate::diagnostics::RecordsBase::RunningImage,
        |_health| {
            let Err(cause) = run_inner() else { return Ok(()) };
            let details = crate::failure_details::Details(cause.as_ref());
            tracing::error!(operation="stable_launcher_failed",cause=%details,"Installed application could not start");
            let copy = sugarsubstitute_contracts::localization::messages("en");
            if let Err(secondary) = sugarsubstitute_desktop::show_startup_failure(
                copy,
                crate::APPLICATION_NAME,
                copy.report.startup_failed,
                format_args!("{details}"),
            ) {
                tracing::error!(operation="stable_launcher_failure_presentation",cause=?secondary,"Startup failure presentation failed");
                return Err(crate::failure_details::retain_presentation(cause, secondary));
            }
            Err(cause)
        },
    );
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(_retained) => ExitCode::FAILURE,
    }
}

/// Run the existing installed-launch lifetime while the caller retains diagnostic routing.
fn run_inner() -> Result<(), Box<dyn Error>> {
    let root = installation_root(&std::env::current_exe()?, std::env::args_os().skip(1))?;
    sugarsubstitute_desktop::run_headless(&mut lifetime::Launcher::new(root))?;
    Ok(())
}

/// The stable executable's parent is the installation identity; callers cannot redirect it.
fn installation_root(
    executable: &Path,
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<PathBuf, Box<dyn Error>> {
    if arguments.into_iter().next().is_some() {
        return Err("Installed launcher does not accept startup overrides".into());
    }
    let root = executable
        .parent()
        .filter(|path| path.is_absolute() && path.parent().is_some())
        .ok_or("Installed launcher must reside in an installation directory")?;
    Ok(root.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn entry_uses_its_own_directory_without_reading_or_creating_state() -> Result<(), Box<dyn Error>>
    {
        assert_eq!(
            installation_root(Path::new(r"E:\unopened\SugarSubstitute.exe"), [])?,
            Path::new(r"E:\unopened")
        );
        Ok(())
    }
    #[test]
    fn entry_cannot_redirect_to_a_foreign_installation_or_use_a_drive_root() {
        assert!(
            installation_root(
                Path::new(r"E:\unopened\SugarSubstitute.exe"),
                [OsString::from("--installation-root")]
            )
            .is_err()
        );
        assert!(installation_root(Path::new(r"E:\SugarSubstitute.exe"), []).is_err());
        assert!(installation_root(Path::new("relative.exe"), []).is_err());
    }
}
