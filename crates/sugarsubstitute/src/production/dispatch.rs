// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Select production explicitly while preserving the existing demo/fixture iterator.
use super::options::ProductionEntryOptions;
use std::{
    error::Error,
    ffi::{OsStr, OsString},
    path::PathBuf,
};
use sugarsubstitute_contracts::configuration::LaunchConfiguration;

/// Retain inert launch inputs; actual namespace discovery checks installed identity.
pub(crate) struct ProductionCommand {
    pub(crate) installation: PathBuf,
    pub(crate) options: ProductionEntryOptions,
}
/// Consume the production prefix only; fallback receives every original argument.
pub(crate) enum Dispatch<I: Iterator<Item = OsString>> {
    Production(ProductionCommand),
    Existing(std::iter::Peekable<I>),
}
/// Parse `--production --installation-root PATH` followed by the six owned options.
/// No filesystem, environment snapshot, execution or native surface is initialized.
/// # Errors
/// Rejects missing installation, invalid Domain path grammar and owned option shape.
pub(crate) fn parse<I: IntoIterator<Item = OsString>>(
    arguments: I,
) -> Result<Dispatch<I::IntoIter>, Box<dyn Error>> {
    let mut arguments = arguments.into_iter().peekable();
    if arguments.peek().is_none_or(|first| first != "--production") {
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
    let options = ProductionEntryOptions::parse(arguments)?;
    Ok(Dispatch::Production(ProductionCommand { installation, options }))
}

/// Run the selected real production role; return the untouched legacy iterator.
/// The actual child constructor is invoked only for a validated production command.
/// # Errors
/// Preserves actual child construction or retained native runner failures.
pub(crate) fn execute<I, C>(
    arguments: I,
    child: impl FnOnce(&ProductionCommand) -> Result<C, Box<dyn Error>>,
) -> Result<Option<std::iter::Peekable<I::IntoIter>>, Box<dyn Error>>
where
    I: IntoIterator<Item = OsString>,
    C: super::entry_runner::EntryActivation + 'static,
{
    match parse(arguments)? {
        Dispatch::Existing(arguments) => Ok(Some(arguments)),
        Dispatch::Production(command) => {
            super::entry_runner::run(command, child)?;
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn executable_fallback_never_constructs_production_child() -> Result<(), Box<dyn Error>> {
        let calls = std::cell::Cell::new(0);
        let legacy = execute([OsString::from("--locale"), OsString::from("ko")], |_| {
            calls.set(calls.get() + 1);
            Ok(super::super::entry_runner::tests::NoChild)
        })?
        .ok_or("Legacy command unexpectedly ran production")?;
        assert_eq!(calls.get(), 0);
        assert_eq!(legacy.collect::<Vec<_>>(), [OsString::from("--locale"), OsString::from("ko")]);
        Ok(())
    }
    #[test]
    fn legacy_fixture_and_demo_arguments_are_preserved_without_collecting()
    -> Result<(), Box<dyn Error>> {
        for arguments in
            [vec!["--internal-native-pipe-child", "--section", "5"], vec!["--locale", "ja"], vec![]]
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
        assert_eq!(command.options.configuration, PathBuf::from(r"E:\configuration.json"));
        Ok(())
    }
}
