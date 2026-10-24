// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Start the installed application by default and select explicit qualification modes.
use super::options::{ConfigurationSource, ProductionEntryOptions};
use std::{
    error::Error,
    ffi::{OsStr, OsString},
    path::PathBuf,
};
use sugarsubstitute_contracts::configuration::LaunchConfiguration;

/// Retain inert launch inputs; actual namespace discovery checks installed identity.
pub(crate) struct ProductionCommand {
    pub(crate) launch_selection:
        Option<sugarsubstitute_supervision::setup_storage::InheritedLaunch>,
    pub(crate) installation: PathBuf,
    pub(crate) diagnostics:
        sugarsubstitute_supervision::coordinator_entry::DiagnosticStorageLocation,
    pub(crate) options: ConfigurationSource,
}
/// Consume the production prefix only; fallback receives every original argument.
pub(crate) enum Dispatch<I: Iterator<Item = OsString>> {
    /// Retain one complete inert command until the runner consumes its original ownership.
    Production(Box<ProductionCommand>),
    Existing(std::iter::Peekable<I>),
}
/// Parse installed production inputs, optional diagnostic root, then the six owned options.
/// No filesystem, environment snapshot, execution or native surface is initialized.
/// # Errors
/// Rejects missing installation, invalid Domain path grammar and owned option shape.
pub(crate) fn parse<I: IntoIterator<Item = OsString>>(
    arguments: I,
) -> Result<Dispatch<I::IntoIter>, Box<dyn Error>> {
    let mut arguments = arguments.into_iter().peekable();
    if arguments.peek().is_none() {
        return Ok(Dispatch::Production(Box::new(installed_command(std::env::current_exe()?)?)));
    }
    if arguments.peek().is_some_and(|first| first != "--production") {
        return Ok(Dispatch::Existing(arguments));
    }
    arguments.next();
    if arguments.next().as_deref() != Some(OsStr::new("--installation-root")) {
        return Err("Production requires an explicit installation root".into());
    }
    let installation =
        PathBuf::from(arguments.next().ok_or("Production installation root missing")?);
    LaunchConfiguration::validate_path(&installation)?;
    if installation.capacity() > 256 * 1024 {
        return Err("Production installation path capacity exceeded".into());
    }
    let diagnostics = if arguments.peek().is_some_and(|value| value == "--diagnostic-root") {
        arguments.next();
        let path = PathBuf::from(arguments.next().ok_or("Production diagnostic root missing")?);
        let location =
            sugarsubstitute_supervision::coordinator_entry::DiagnosticStorageLocation::Explicit(
                path,
            );
        location.validate()?;
        location
    } else {
        sugarsubstitute_supervision::coordinator_entry::DiagnosticStorageLocation::Account
    };
    let options = ProductionEntryOptions::parse(arguments)?;
    Ok(Dispatch::Production(Box::new(ProductionCommand {
        launch_selection: None,
        installation,
        diagnostics,
        options: options.into(),
    })))
}

/// Build inert installed paths without disk IO before the first splash frame.
pub(super) fn installed_command(executable: PathBuf) -> Result<ProductionCommand, Box<dyn Error>> {
    let installation =
        executable.parent().ok_or("Installed executable has no directory")?.to_path_buf();
    LaunchConfiguration::validate_path(&installation)?;
    Ok(ProductionCommand {
        launch_selection: None,
        diagnostics:
            sugarsubstitute_supervision::coordinator_entry::DiagnosticStorageLocation::Account,
        options: ConfigurationSource::Installed(
            installation.join("sugarsubstitute-launch.local.json"),
        ),
        installation,
    })
}

/// Run the selected real production role; return the untouched legacy iterator.
/// The actual child constructor is invoked only for a validated production command.
/// # Errors
/// Preserves actual child construction or retained native runner failures.
pub(crate) fn execute<I, C>(
    arguments: I,
    diagnostics: crate::diagnostics::DiagnosticHealth,
    child: impl FnOnce(&ProductionCommand) -> Result<Box<C>, Box<dyn Error>>,
) -> Result<Option<std::iter::Peekable<I::IntoIter>>, Box<dyn Error>>
where
    I: IntoIterator<Item = OsString>,
    C: super::entry_runner::EntryActivation + 'static,
{
    match parse(arguments)? {
        Dispatch::Existing(arguments) => Ok(Some(arguments)),
        Dispatch::Production(command) => {
            super::entry_runner::run_application(*command, child, diagnostics)?;
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_arguments_select_real_installed_startup_without_reading_files()
    -> Result<(), Box<dyn Error>> {
        let Dispatch::Production(command) = parse(Vec::<OsString>::new())? else {
            return Err("Double click selected the splash demo".into());
        };
        let ConfigurationSource::Installed(path) = command.options else {
            return Err("Installed startup descriptor was not selected".into());
        };
        assert_eq!(path, command.installation.join("sugarsubstitute-launch.local.json"));
        assert!(command.diagnostics.explicit_path().is_none());
        let command = installed_command(PathBuf::from(r"E:\unopened\sugarsubstitute.exe"))?;
        assert_eq!(command.installation, std::path::Path::new(r"E:\unopened"));
        Ok(())
    }
    #[test]
    fn executable_fallback_never_constructs_production_child() -> Result<(), Box<dyn Error>> {
        let calls = std::cell::Cell::new(0);
        let legacy = execute(
            [OsString::from("--locale"), OsString::from("ko")],
            crate::diagnostics::DiagnosticHealth::fixture(),
            |_| {
                calls.set(calls.get() + 1);
                Ok(Box::new(super::super::entry_runner::tests::NoChild(None)))
            },
        )?
        .ok_or("Legacy command unexpectedly ran production")?;
        assert_eq!(calls.get(), 0);
        assert_eq!(legacy.collect::<Vec<_>>(), [OsString::from("--locale"), OsString::from("ko")]);
        Ok(())
    }
    #[test]
    fn legacy_fixture_and_demo_arguments_are_preserved_without_collecting()
    -> Result<(), Box<dyn Error>> {
        for arguments in
            [vec!["--internal-native-pipe-child", "--section", "5"], vec!["--locale", "ja"]]
        {
            let expected: Vec<_> = arguments.into_iter().map(OsString::from).collect();
            let Dispatch::Existing(original) = parse(expected.clone())? else {
                return Err("Legacy command was changed".into());
            };
            assert_eq!(original.collect::<Vec<_>>(), expected);
        }
        Ok(())
    }
    #[test]
    fn production_requires_explicit_valid_installation_before_option_parsing() {
        assert!(parse(["--production"].map(OsString::from)).is_err());
        assert!(
            parse(["--production", "--installation-root", "relative"].map(OsString::from)).is_err()
        );
    }
    #[test]
    fn production_keeps_exact_existing_six_option_grammar() -> Result<(), Box<dyn Error>> {
        let Dispatch::Production(command) = parse(
            [
                "--production",
                "--installation-root",
                r"E:\installation",
                "--startup-config",
                r"E:\configuration.json",
                "--frontend-package",
                r"E:\frontend",
                "--frontend-metadata",
                r"E:\metadata",
                "--sugarcubes-root",
                r"E:\cubes",
                "--backend-script",
                r"E:\main.py",
                "--profile-directory",
                r"E:\profile",
            ]
            .map(OsString::from),
        )?
        else {
            return Err("Production prefix was ignored".into());
        };
        assert_eq!(command.installation, PathBuf::from(r"E:\installation"));
        let ConfigurationSource::Explicit(paths) = command.options else {
            return Err("Explicit launch paths missing".into());
        };
        assert_eq!(paths.configuration, PathBuf::from(r"E:\configuration.json"));
        Ok(())
    }
}

#[cfg(test)]
mod diagnostic_location_tests {
    use super::*;
    fn arguments() -> Vec<OsString> {
        [
            "--production",
            "--installation-root",
            r"D:\installed",
            "--diagnostic-root",
            r"D:\private-diagnostics",
            "--startup-config",
            r"D:\startup.toml",
            "--frontend-package",
            r"D:\frontend",
            "--frontend-metadata",
            r"D:\metadata",
            "--sugarcubes-root",
            r"D:\cubes",
            "--backend-script",
            r"D:\main.py",
            "--profile-directory",
            r"D:\profile",
        ]
        .map(OsString::from)
        .into()
    }
    #[test]
    fn explicit_diagnostic_root_is_local_desktop_configuration_and_omission_preserves_account()
    -> Result<(), Box<dyn Error>> {
        let Dispatch::Production(command) = parse(arguments())? else {
            return Err("Production not selected".into());
        };
        assert_eq!(
            command.diagnostics.explicit_path(),
            Some(std::path::Path::new(r"D:\private-diagnostics"))
        );
        let mut omitted = arguments();
        omitted.drain(3..5);
        let Dispatch::Production(command) = parse(omitted)? else {
            return Err("Production not selected".into());
        };
        assert!(command.diagnostics.explicit_path().is_none());
        Ok(())
    }
    #[test]
    fn malformed_or_duplicate_diagnostic_root_rejects_before_production_activation() {
        for path in ["relative", r"\\server\share\private", r"D:\"] {
            let mut input = arguments();
            input[4] = OsString::from(path);
            assert!(parse(input).is_err());
        }
        let mut duplicate = arguments();
        duplicate.splice(5..5, ["--diagnostic-root", r"D:\second"].map(OsString::from));
        assert!(parse(duplicate).is_err());
        assert!(
            parse(
                ["--production", "--installation-root", r"D:\installed", "--diagnostic-root"]
                    .map(OsString::from)
            )
            .is_err()
        );
    }
}
