// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Dedicated updater process entry shares the admitted preparation session with historical hosts.
use std::{error::Error, ffi::OsString, io, process::ExitCode, sync::Arc};
use sugarsubstitute_contracts::application_launch::{
    ENGINE_LAUNCH_ARGUMENT,
    updater::{PREPARATION_ARGUMENT, preparation::HelperExit, probe::PROBE_ARGUMENT},
};
use sugarsubstitute_supervision::application_launch::EngineLaunch;
use sugarsubstitute_supervision::setup_storage::InheritedUpdaterProbe;

/// Run once on the native main thread, retiring owned jobs before returning process status.
/// Each role reports status to its owning process; failed native work settles before return.
pub fn entry(
    probe: io::Result<Option<InheritedUpdaterProbe>>,
    engine: io::Result<Option<EngineLaunch>>,
) -> ExitCode {
    let result = crate::diagnostics::Diagnostics::run(
        crate::diagnostics::RecordsBase::RunningImage,
        |_health| {
            let result = (|| {
                let probe = probe?.map(Arc::new);
                let engine = engine?;
                arguments(std::env::args_os().skip(1)).and_then(|(role, arguments)| {
                    match (role, &probe, &engine) {
                        (Role::Engine, None, Some(engine)) => super::activation::run(engine),
                        (Role::Preparation, None, None) => super::run(arguments),
                        (Role::Probe, Some(probe), None) => super::probe::run(Arc::clone(probe)),
                        _ => Err("Updater entry and inherited custody differ".into()),
                    }
                })
            })();
            if let Err(cause) = &result {
                tracing::error!(operation="updater_entry_failed",cause=%crate::failure_details::Details(cause.as_ref()),"Update worker failed");
            }
            result
        },
    );
    let class = match result {
        Ok(()) => HelperExit::Retired,
        Err(_retained) => HelperExit::Failed,
    };
    ExitCode::from(u8::try_from(class.code()).unwrap_or(u8::MAX))
}

/// Exact public entry selection prevents desktop or installer modes in the updater executable.
#[derive(Debug, PartialEq, Eq)]
enum Role {
    Engine,
    Preparation,
    Probe,
}

/// Select an exact public entry spelling while preserving every subsequent argument.
fn arguments(
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<(Role, impl Iterator<Item = OsString>), Box<dyn Error>> {
    let mut arguments = arguments.into_iter();
    let role = match arguments.next().as_deref() {
        Some(value) if value == std::ffi::OsStr::new(ENGINE_LAUNCH_ARGUMENT) => Role::Engine,
        Some(value) if value == std::ffi::OsStr::new(PREPARATION_ARGUMENT) => Role::Preparation,
        Some(value) if value == std::ffi::OsStr::new(PROBE_ARGUMENT) => Role::Probe,
        _ => return Err("An explicit updater entry is required".into()),
    };
    Ok((role, arguments))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedicated_entry_requires_the_declared_role_and_preserves_path_arguments()
    -> Result<(), Box<dyn Error>> {
        let paths =
            [OsString::from(r"E:\installation with spaces"), OsString::from(r"E:\workspace")];
        for (entry, expected) in [
            (PREPARATION_ARGUMENT, Role::Preparation),
            (PROBE_ARGUMENT, Role::Probe),
            (ENGINE_LAUNCH_ARGUMENT, Role::Engine),
        ] {
            let input = [OsString::from(entry)].into_iter().chain(paths.clone());
            let (role, values) = arguments(input)?;
            assert_eq!(role, expected);
            assert_eq!(values.collect::<Vec<_>>(), paths);
        }
        for input in
            [vec![], vec![OsString::from("--setup")], vec![OsString::from("--installed-launch-v1")]]
        {
            assert!(arguments(input).is_err());
        }
        Ok(())
    }
}
