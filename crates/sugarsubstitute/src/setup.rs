// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose the branded installer with isolated upstream provisioning and native ownership.
mod console;
mod controller;
mod distribution;
mod failure;
pub mod presentation;
mod worker;
use std::{error::Error, path::PathBuf};

/// Run the native installer entry point or its off-screen qualification.
///
/// The bundle directory contains the pinned worker and application payload. Qualification modes
/// exercise owned off-screen lifetimes.
/// `distribution` must be the authenticated producing installer's compiled configuration bytes.
/// # Errors
/// Preserves invalid command, filesystem, native UI and installation failures.
pub fn run(distribution: &'static [u8]) -> Result<(), Box<dyn Error>> {
    let interactive = !std::env::args_os().skip(1).any(|value| {
        matches!(
            value.to_str(),
            Some(
                "--qualify-lifecycle"
                    | "--qualify-window"
                    | "--qualify-managed"
                    | "--qualify-cancel"
                    | "--qualify-maintenance"
                    | "--qualify-launch"
                    | "--qualify-maintenance-launch"
            )
        )
    });
    crate::diagnostics::Diagnostics::run(|_health| {
        let mut locale = String::from("en");
        let result = run_inner(distribution, &mut locale);
        failure::report(result, &locale, interactive, sugarsubstitute_desktop::show_startup_failure)
    })
}

fn run_inner(distribution: &'static [u8], locale: &mut String) -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let mut bundle = std::env::current_exe()?;
    let mut workspace = None;
    let mut qualification = None;
    let mut qualify_window = false;
    let mut qualify_managed = None;
    let mut qualify_maintenance = false;
    let mut qualify_launch = false;
    let mut existing = None;
    let mut native_installation = None;
    while let Some(argument) = arguments.next() {
        let value = arguments.next().ok_or("Installer option requires a value")?;
        match argument.to_str() {
            Some("--bundle") => bundle = PathBuf::from(value),
            Some("--workspace") => workspace = Some(PathBuf::from(value)),
            Some("--qualify-lifecycle") => qualification = Some(PathBuf::from(value)),
            Some("--qualify-launch") => {
                qualification = Some(PathBuf::from(value));
                qualify_launch = true;
            }
            Some("--qualify-maintenance" | "--qualify-maintenance-launch") => {
                qualification = Some(PathBuf::from(value));
                qualify_maintenance = true;
                qualify_launch = argument == "--qualify-maintenance-launch";
            }
            Some("--qualify-window") => {
                qualification = Some(PathBuf::from(value));
                qualify_window = true;
            }
            Some("--qualify-managed" | "--qualify-cancel") => {
                qualification = Some(PathBuf::from(value));
                qualify_managed = Some(argument == "--qualify-cancel");
            }
            Some("--existing-comfy") => existing = Some(PathBuf::from(value)),
            Some("--native-installation") => native_installation = Some(PathBuf::from(value)),
            _ => return Err("Unknown installer option".into()),
        }
    }
    if qualify_maintenance != native_installation.is_some()
        || (qualify_launch && (qualify_window || qualify_managed.is_some()))
        || (qualify_launch && !qualify_maintenance && existing.is_none())
        || (qualify_maintenance
            && (qualify_window || qualify_managed.is_some() || existing.is_some()))
    {
        return Err("Maintenance qualification requires only a native installation target".into());
    }
    let locations =
        sugarsubstitute_configuration::installer::InstallerLocations::capture(workspace)?;
    let workspace = locations.workspace().to_path_buf();
    if !bundle.is_absolute() || !workspace.is_absolute() {
        return Err("Installer paths must be absolute".into());
    }
    if let Some(output) = qualification {
        let result = if let Some(cancel) = qualify_managed {
            controller::managed_qualification::run(bundle, workspace, cancel, distribution)
        } else if qualify_window {
            controller::window_qualification::run(bundle, workspace, distribution)
        } else {
            controller::qualification::run(
                bundle,
                workspace,
                existing,
                native_installation,
                qualify_launch,
                distribution,
            )
        };
        let receipt = match &result {
            Ok(value) => value.clone(),
            Err(cause) => {
                let mut causes = Vec::new();
                let mut current: Option<&dyn Error> = Some(cause.as_ref());
                while let Some(cause) = current {
                    causes.push(cause.to_string());
                    current = cause.source();
                    if causes.len() == 16 {
                        break;
                    }
                }
                serde_json::json!({"schema_version":1,"passed":false,"causes":causes})
            }
        };
        crate::artifact_job::write_json(output, receipt, false)?;
        result?;
        return Ok(());
    }
    let mut controller = controller::Native::new(
        bundle,
        workspace,
        distribution,
        locations
            .install_root()
            .to_str()
            .ok_or("Installation directory is not Unicode")?
            .to_owned(),
    )?;
    let result = sugarsubstitute_desktop::installer::native::run(
        sugarsubstitute_desktop::installer::native::Window {
            visible: true,
            title: sugarsubstitute_contracts::localization::installer::messages("en").title.into(),
            close: presentation::Action::Close.id(),
        },
        &mut controller,
    );
    *locale = controller.locale().to_owned();
    result
}
