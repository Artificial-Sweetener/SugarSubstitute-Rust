// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose the branded installer with isolated upstream provisioning and native ownership.
mod console;
mod controller;
mod distribution;
mod failure;
mod surface;
mod worker;
use std::{error::Error, path::PathBuf};

/// Run the installer entry point, or export its exact authored surface for off-screen qualification.
///
/// The bundle directory contains the pinned worker and application payload. Surface exports
/// write the authored document; qualification modes exercise owned off-screen lifetimes.
/// `distribution` must be the authenticated producing installer's compiled configuration bytes.
/// # Errors
/// Preserves invalid command, filesystem, native UI and installation failures.
pub fn run(distribution: &'static [u8]) -> Result<(), Box<dyn Error>> {
    let interactive = !std::env::args_os().skip(1).any(|value| {
        matches!(
            value.to_str(),
            Some(
                "--export-surface"
                    | "--qualify-lifecycle"
                    | "--qualify-window"
                    | "--qualify-managed"
                    | "--qualify-cancel"
                    | "--qualify-maintenance"
            )
        )
    });
    failure::report(
        run_inner(distribution),
        interactive,
        sugarsubstitute_desktop::show_startup_failure,
    )
}

fn run_inner(distribution: &'static [u8]) -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let mut bundle = std::env::current_exe()?;
    let mut workspace = None;
    let mut export = None;
    let mut qualification = None;
    let mut qualify_window = false;
    let mut qualify_managed = None;
    let mut qualify_maintenance = false;
    let mut existing = None;
    let mut native_installation = None;
    while let Some(argument) = arguments.next() {
        let value = arguments.next().ok_or("Installer option requires a value")?;
        match argument.to_str() {
            Some("--bundle") => bundle = PathBuf::from(value),
            Some("--workspace") => workspace = Some(PathBuf::from(value)),
            Some("--export-surface") => export = Some(PathBuf::from(value)),
            Some("--qualify-lifecycle") => qualification = Some(PathBuf::from(value)),
            Some("--qualify-maintenance") => {
                qualification = Some(PathBuf::from(value));
                qualify_maintenance = true;
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
        || (qualify_maintenance
            && (qualify_window || qualify_managed.is_some() || existing.is_some()))
    {
        return Err("Maintenance qualification requires only a native installation target".into());
    }
    let locations =
        sugarsubstitute_configuration::installer::InstallerLocations::capture(workspace)?;
    let workspace = locations.workspace().to_path_buf();
    let html = surface::html("en", locations.install_root())?;
    if let Some(export) = export {
        std::fs::write(export, html)?;
        return Ok(());
    }
    if !bundle.is_absolute() || !workspace.is_absolute() {
        return Err("Installer paths must be absolute".into());
    }
    if let Some(output) = qualification {
        let diagnostics = crate::diagnostics::Diagnostics::initialize()?;
        let result = diagnostics.finish(if let Some(cancel) = qualify_managed {
            controller::managed_qualification::run(bundle, workspace, cancel, distribution)
        } else if qualify_window {
            controller::window_qualification::run(bundle, workspace, distribution)
        } else {
            controller::qualification::run(
                bundle,
                workspace,
                existing,
                native_installation,
                distribution,
            )
        });
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
        std::fs::write(output, serde_json::to_vec_pretty(&receipt)?)?;
        result?;
        return Ok(());
    }
    let diagnostics = crate::diagnostics::Diagnostics::initialize()?;
    let mut controller = controller::Controller::new(bundle, workspace.clone(), distribution);
    diagnostics.finish(sugarsubstitute_desktop::installer::run(
        sugarsubstitute_desktop::installer::InstallerWindow {
            visible: true,
            title: sugarsubstitute_contracts::localization::installer::messages("en")["title"]
                .into(),
            html,
        },
        &mut controller,
    ))
}
