// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Stable installed entry point; the selected application owns the only visible startup window.
//!
//! Run from the installation root with no arguments, this is the stable entry: it starts the
//! launch engine the launch pointer names and exits. Run from a generation folder with the
//! engine argument, it is that generation's launch engine and owns the rest of startup.
mod lifetime;
mod root;
#[cfg(test)]
mod test_support;
use std::{
    error::Error,
    ffi::OsString,
    path::{Path, PathBuf},
    process::ExitCode,
};

/// Run a newer accepted updater as the launch engine once, authenticating its own image first.
/// Called on the native main thread under the updater entry's diagnostics lifetime.
pub(crate) fn run_update_engine(
    installation: PathBuf,
    executable: PathBuf,
    repository: [u8; 32],
) -> Result<(), Box<dyn Error>> {
    sugarsubstitute_desktop::run_headless(&mut lifetime::Launcher::update_engine(
        installation,
        executable,
        repository,
    ))?;
    Ok(())
}

/// Run beside the installation record without acquiring the application run lock.
/// Must be called once on the native main thread. Selected application startup is independent
/// after creation; this process retires its own worker scopes and inherited-handle duplicates.
pub fn run(
    engine: std::io::Result<Option<sugarsubstitute_supervision::application_launch::EngineLaunch>>,
) -> ExitCode {
    let result = crate::diagnostics::Diagnostics::run(
        crate::diagnostics::RecordsBase::RunningImage,
        |_health| {
            let Err(cause) = run_inner(engine) else { return Ok(()) };
            let details = crate::failure_details::Details(cause.as_ref());
            tracing::error!(operation="stable_launcher_failed",cause=%details,"Installed application could not start");
            let copy = sugarsubstitute_contracts::localization::messages("en");
            if let Err(secondary) = sugarsubstitute_desktop::show_startup_failure(
                copy,
                crate::APPLICATION_NAME,
                if needs_reinstall(cause.as_ref()) {
                    copy.report.reinstall_needed
                } else {
                    copy.report.startup_failed
                },
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

/// Run the stable entry or this generation's launch engine while the caller retains diagnostics.
fn run_inner(
    engine: std::io::Result<Option<sugarsubstitute_supervision::application_launch::EngineLaunch>>,
) -> Result<(), Box<dyn Error>> {
    let executable = std::env::current_exe()?;
    match engine? {
        Some(launch) => {
            let root = engine_installation(&executable, launch.installation())?;
            let generation = executable
                .parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                .ok_or("Launch engine generation folder is unnamed")?;
            let generation =
                sugarsubstitute_contracts::installation::GenerationId::new(generation.to_owned())?;
            sugarsubstitute_desktop::run_headless(&mut lifetime::Launcher::engine(
                root,
                generation,
                launch.hops(),
            ))?;
        }
        None => {
            let root = installation_root(&executable, std::env::args_os().skip(1))?;
            sugarsubstitute_desktop::run_headless(&mut root::Root::new(root))?;
        }
    }
    Ok(())
}

/// A launch engine runs from `<installation>\releases\<generation>` of the installation its
/// creator named; any other placement is refused.
fn engine_installation(executable: &Path, named: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let releases = executable
        .parent()
        .and_then(Path::parent)
        .filter(|releases| {
            releases.file_name().is_some_and(|name| name.eq_ignore_ascii_case("releases"))
        })
        .ok_or("Launch engine must run from a generation folder")?;
    let installation = releases.parent().ok_or("Launch engine installation is absent")?;
    if !installation.as_os_str().eq_ignore_ascii_case(named.as_os_str()) {
        return Err("Launch engine belongs to a different installation".into());
    }
    Ok(installation.to_path_buf())
}

/// Whether startup stopped because the installation needs reinstalling rather than a retry: the
/// launch pointer is missing, damaged or in a format this release cannot read, no pointed
/// engine could take over, the installation has no accepted release, or the selection record
/// uses an incompatible schema. Reinstalling keeps user files and settings.
fn needs_reinstall(cause: &(dyn Error + 'static)) -> bool {
    use sugarsubstitute_contracts::installation::Rejection;
    use sugarsubstitute_supervision::{
        application_launch::StartupRefused,
        setup_storage::{PointerFailure, RouteFailure, SelectionFailure},
    };
    let mut next = Some(cause);
    while let Some(current) = next {
        if let Some(inner) =
            current.downcast_ref::<std::io::Error>().and_then(std::io::Error::get_ref)
            && needs_reinstall(inner)
        {
            return true;
        }
        if matches!(
            current.downcast_ref::<StartupRefused>(),
            Some(
                StartupRefused::Pointer(
                    PointerFailure::Missing
                        | PointerFailure::Incompatible
                        | PointerFailure::Damaged
                ) | StartupRefused::NoEngine(_)
            )
        ) || matches!(
            current.downcast_ref::<RouteFailure>(),
            Some(
                RouteFailure::NeedsInstallation
                    | RouteFailure::Selection(SelectionFailure::Record(Rejection::Schema))
            )
        ) {
            return true;
        }
        next = current.source();
    }
    false
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
    fn an_engine_runs_only_from_a_generation_of_the_installation_its_creator_named()
    -> Result<(), Box<dyn Error>> {
        let engine = Path::new(r"E:\Sugar\releases\0123\sugarsubstitute-launcher.exe");
        assert_eq!(engine_installation(engine, Path::new(r"e:\sugar"))?, Path::new(r"E:\Sugar"));
        assert!(engine_installation(engine, Path::new(r"E:\Other")).is_err());
        for misplaced in [
            r"E:\Sugar\sugarsubstitute-launcher.exe",
            r"E:\Sugar\other\0123\sugarsubstitute-launcher.exe",
        ] {
            assert!(engine_installation(Path::new(misplaced), Path::new(r"E:\Sugar")).is_err());
        }
        Ok(())
    }
    #[test]
    fn an_unreadable_installation_asks_for_a_reinstall_and_other_failures_do_not() {
        use sugarsubstitute_contracts::installation::Rejection;
        use sugarsubstitute_supervision::{
            application_launch::StartupRefused,
            setup_storage::{PointerFailure, RouteFailure, SelectionFailure},
        };
        let wrapped = |cause: Box<dyn Error + Send + Sync>| -> Box<dyn Error> {
            Box::new(std::io::Error::other(cause))
        };
        for cause in [
            wrapped(Box::new(StartupRefused::Pointer(PointerFailure::Incompatible))),
            wrapped(Box::new(StartupRefused::Pointer(PointerFailure::Missing))),
            wrapped(Box::new(StartupRefused::Pointer(PointerFailure::Damaged))),
            wrapped(Box::new(StartupRefused::NoEngine(Vec::new()))),
            wrapped(Box::new(RouteFailure::Selection(SelectionFailure::Record(Rejection::Schema)))),
            wrapped(Box::new(RouteFailure::NeedsInstallation)),
        ] {
            assert!(needs_reinstall(cause.as_ref()), "{cause}");
        }
        for cause in [
            wrapped(Box::new(StartupRefused::Pointer(PointerFailure::Storage(
                std::io::Error::other("disk"),
            )))),
            wrapped(Box::new(RouteFailure::Foreign)),
            wrapped(Box::new(RouteFailure::Selection(SelectionFailure::LockTimeout))),
            Box::new(std::io::Error::other("storage failed")) as Box<dyn Error>,
        ] {
            assert!(!needs_reinstall(cause.as_ref()), "{cause}");
        }
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
