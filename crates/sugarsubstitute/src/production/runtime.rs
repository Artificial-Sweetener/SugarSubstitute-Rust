// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Select the real coordinator or inherited desktop role and compose its owned runtime.
use super::{
    child::{ChildInputs, ProductionChild},
    entry_activation::{ProductionActivation, desktop_role::DesktopRole},
    startup::{ProductionStartup, StartupScopes},
};
use crate::job_sequence::JobSequence;
use std::{error::Error, ffi::OsString};
use sugarsubstitute_contracts::{
    execution::{ScopeId, ScopeSpec},
    localization::messages,
};
use sugarsubstitute_desktop::{DesktopOptions, MainCaptionLabels, MainWindowSettings};
use sugarsubstitute_execution::MonotonicClock;

/// Execute a dedicated real role, preserving every legacy argument on fallback.
/// Inherited role spelling grants no authority; the captured native transport verifies it.
/// # Errors
/// Preserve lexical, actual startup, coordinator and native teardown failures.
pub(crate) fn execute<I: IntoIterator<Item = OsString>>(
    arguments: I,
    diagnostics: crate::diagnostics::DiagnosticHealth,
) -> Result<Option<impl Iterator<Item = OsString>>, Box<dyn Error>> {
    let mut arguments = arguments.into_iter().peekable();
    if arguments.peek().is_some_and(|value| value == "--internal-backend-custody") {
        super::backend_custody::run(arguments)?;
        return Ok(None);
    }
    if arguments.peek().is_some_and(|value| value == "--internal-production-reporter") {
        super::reporter_role::runtime::run(super::reporter_role::options::ReporterRole::parse(
            arguments,
        )?)?;
        return Ok(None);
    }
    let qualification = super::qualification::QualificationIntent::root(&mut arguments)?;
    if arguments.peek().is_some_and(|value| value == "--internal-production-desktop") {
        run_desktop(DesktopRole::parse(arguments)?, diagnostics)?;
        return Ok(None);
    }
    super::dispatch::execute(arguments, diagnostics, |command| {
        ProductionActivation::new(
            std::env::current_exe()?,
            command.installation.clone(),
            command.installation.join("capture"),
        )
        .map(|activation| activation.with_qualification(qualification))
    })
}

/// Compose one actual captured child before the native adapter admits any surface.
/// Scope identities belong to this process; jobs retain original per-operation deadlines.
/// # Errors
/// Retain failures from actual capture, configuration, backend, main and final joins.
fn run_desktop(
    role: DesktopRole,
    diagnostics: crate::diagnostics::DiagnosticHealth,
) -> Result<(), Box<dyn Error>> {
    let labels = messages("en").window;
    let child = ProductionChild::new(ChildInputs {
        diagnostics,
        qualification: role.qualification,
        input: role.bootstrap,
        original: role.original,
        run: role.run,
        native_scope: ScopeSpec { id: ScopeId::new(3)?, deadline: None },
        identity: (role.launch, role.navigation),
        settings: MainWindowSettings {
            logical_width: 1280,
            logical_height: 800,
            dark: true,
            labels: MainCaptionLabels {
                minimize: labels.minimize,
                maximize: labels.maximize,
                restore: labels.restore,
                close: labels.close,
            },
        },
    })?;
    let content = ProductionStartup::new(
        role.run,
        role.launch,
        child,
        MonotonicClock::new(),
        JobSequence::new(),
        StartupScopes {
            child: ScopeSpec { id: ScopeId::new(1)?, deadline: None },
            render: ScopeSpec { id: ScopeId::new(2)?, deadline: None },
        },
    )?;
    let receipt = sugarsubstitute_desktop::run_startup(
        DesktopOptions {
            title: sugarsubstitute::APPLICATION_NAME.into(),
            logical_width: 558,
            logical_height: 558,
            dark: true,
            duration: None,
            audit_frames: false,
            audit_timings: false,
        },
        content,
    )?;
    #[cfg(feature = "k1-native-qualification")]
    sugarsubstitute_desktop::qualification_trace::finished();
    tracing::info!(operation = "production_desktop_retired", navigations = receipt.navigations_started, main_material = ?receipt.main_material, splash_frames = receipt.splash.frames_presented, "SugarSubstitute desktop retired");
    Ok(())
}
