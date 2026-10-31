// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Select the real coordinator or inherited desktop role and compose its owned runtime.
use super::{
    child::{ChildInputs, ProductionChild},
    entry_activation::{ProductionActivation, desktop_role::DesktopRole},
    scope_sequence::ScopeSequence,
};
use crate::job_sequence::JobSequence;
use crate::startup::{StartupHost, StartupScopes};
use std::{error::Error, ffi::OsString};
use sugarsubstitute_contracts::{execution::ScopeSpec, localization::messages};
use sugarsubstitute_desktop::{DesktopOptions, MainCaptionLabels, MainWindowSettings};
use sugarsubstitute_execution::MonotonicClock;

/// Start the accepted or trial image using custody captured at the executable entry boundary.
/// The actual coordinator worker validates its identity; forwarding keeps it through retirement.
pub(crate) fn execute_inherited(
    selection: std::io::Result<sugarsubstitute_supervision::setup_storage::InheritedLaunch>,
    diagnostics: crate::diagnostics::DiagnosticHealth,
) -> Result<(), Box<dyn Error>> {
    let selection = selection.map_err(|cause| {
        super::entry_failure::report_failure::<ProductionActivation, _>(
            Box::new(cause),
            sugarsubstitute_desktop::show_startup_failure,
        )
    })?;
    let mut command = super::dispatch::installed_command(std::env::current_exe()?)?;
    command.launch_selection = Some(selection);
    super::entry_runner::run_application(
        command,
        |command| {
            ProductionActivation::new(
                std::env::current_exe()?,
                command.installation.clone(),
                command.installation.join("capture"),
            )
        },
        diagnostics,
    )
}

/// Execute a dedicated real role, preserving every legacy argument on fallback.
/// Inherited role spelling grants no authority; the captured native transport verifies it.
/// # Errors
/// Preserve lexical, actual startup, coordinator and native teardown failures.
pub(crate) fn execute<I: IntoIterator<Item = OsString>>(
    arguments: I,
    diagnostics: crate::diagnostics::DiagnosticHealth,
) -> Result<Option<impl Iterator<Item = OsString>>, Box<dyn Error>> {
    let mut arguments = arguments.into_iter().peekable();
    if arguments.peek().is_some_and(|value| {
        value == sugarsubstitute_contracts::application_launch::updater::PREPARATION_ARGUMENT
    }) {
        arguments.next();
        crate::updates::run(arguments)?;
        return Ok(None);
    }
    if arguments.peek().is_some_and(|value| value == "--internal-profile-custody-component") {
        arguments.next();
        let command = super::entry_activation::ProfileComponent::command(arguments)?;
        super::entry_runner::run(
            command,
            |command| {
                super::entry_activation::ProfileComponent::new(
                    std::env::current_exe()?,
                    command.installation.clone(),
                )
            },
            diagnostics,
        )?;
        return Ok(None);
    }
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
    if arguments.peek().is_some_and(|value| super::recovery_supervisor::role::matches_prefix(value))
    {
        super::recovery_supervisor::run(arguments)?;
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
    let scopes = ScopeSequence::new();
    let child_scope = ScopeSpec { id: scopes.allocate()?, deadline: None };
    let render_scope = ScopeSpec { id: scopes.allocate()?, deadline: None };
    let native_scope = ScopeSpec { id: scopes.allocate()?, deadline: None };
    #[cfg(feature = "native-composition")]
    let composition_scope = ScopeSpec { id: scopes.allocate()?, deadline: None };
    let canvas_scope = ScopeSpec { id: scopes.allocate()?, deadline: None };
    let canvas_health = diagnostics.clone();
    let child = ProductionChild::new(ChildInputs {
        diagnostics,
        qualification: role.qualification,
        input: role.bootstrap,
        original: role.original,
        run: role.run,
        native_scope,
        identity: (role.launch, role.navigation),
        settings: MainWindowSettings {
            recovery: sugarsubstitute_desktop::MainRecoveryLabels {
                retry: sugarsubstitute_contracts::localization::messages("en").retry,
                retry_ready: sugarsubstitute_contracts::localization::messages("en").retry_ready,
            },
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
    let mut content = StartupHost::new(
        role.run,
        role.launch,
        child,
        MonotonicClock::new(),
        JobSequence::new(),
        StartupScopes { child: child_scope, render: render_scope },
    )?;
    #[cfg(feature = "native-composition")]
    content.configure_composition(composition_scope)?;
    // The parent's installation root anchors the canvas's runtime cache scope, beside the
    // installation's other runtime cache consumers. The cache owner validates it on a worker.
    content.configure_canvas(role.installation, canvas_health, canvas_scope)?;
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
