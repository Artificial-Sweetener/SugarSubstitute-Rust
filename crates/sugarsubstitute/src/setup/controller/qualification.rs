// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify installer preparation and optional actual application creation on an offscreen desktop.
use super::*;
use std::{
    num::NonZeroUsize,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::CompletionWake;
use sugarsubstitute_desktop::{HeadlessLifetime, HeadlessStep, StartupPreparationStep};

pub(in crate::setup) fn run(
    bundle: PathBuf,
    workspace: PathBuf,
    existing: Option<PathBuf>,
    native_installation: Option<PathBuf>,
    launch: bool,
    distribution: &'static [u8],
) -> Result<Value> {
    let install_root = native_installation.clone().unwrap_or_else(|| workspace.join("installed"));
    let mut fixture = Fixture {
        controller: Controller::new(bundle, workspace.clone(), distribution),
        session: session::Session::new(
            events::path_text(install_root)?,
            crate::setup::presentation::View {
                surface: sugarsubstitute_ui::text::PaintSpec {
                    width: 1180,
                    height: 760,
                    scale: 1.0,
                    theme: sugarsubstitute_contracts::configuration::Theme::Dark,
                },
                scroll: 0.0,
                now: Duration::ZERO,
                interaction: crate::setup::presentation::Interaction::default(),
                text_revisions: [0; 5],
                accent: sugarsubstitute_ui::accent::AccentPalette::PRODUCT,
            },
        )?,
        workspace,
        existing,
        native_installation,
        phase: Phase::Preparing,
        rejected_invalid: false,
        inspected: false,
        installed: None,
        launch,
        launched: false,
        started: Instant::now(),
    };
    sugarsubstitute_desktop::run_headless(&mut fixture)?;
    let passed = if fixture.native_installation.is_some() {
        fixture.inspected && fixture.installed.is_some() && fixture.controller.worker.is_none()
    } else {
        fixture.rejected_invalid
            && (fixture.existing.is_none() || (fixture.inspected && fixture.installed.is_some()))
    };
    if !passed || (fixture.launch && !fixture.launched) {
        return Err("Installer qualification did not observe every required result".into());
    }
    Ok(json!({
        "schema_version":1, "passed":true, "invalid_environment_rejected":fixture.rejected_invalid,
        "existing_environment_inspected":fixture.inspected,
        "installed_executable":fixture.installed, "process_family_and_execution_settled":true,
        "settlement_scope":"installer_workers",
        "native_maintenance":fixture.native_installation.is_some(),
        "application_creation_observed":fixture.launched,
    }))
}

/// Boundary rejection is qualified before the real native navigation admits installation.
enum Phase {
    Preparing,
    Invalid,
    Native,
}
struct Fixture {
    controller: Controller,
    session: session::Session,
    workspace: PathBuf,
    existing: Option<PathBuf>,
    native_installation: Option<PathBuf>,
    phase: Phase,
    rejected_invalid: bool,
    inspected: bool,
    installed: Option<PathBuf>,
    launch: bool,
    launched: bool,
    started: Instant,
}
impl Fixture {
    /// Use the native flow's commands and result correlation throughout the installation route.
    fn progress(&mut self) -> Result<HeadlessStep> {
        if self.controller.closing {
            self.session.advance(&mut self.controller, self.started.elapsed())?;
            return Ok(HeadlessStep::Shutdown);
        }
        if self.started.elapsed() >= Duration::from_secs(300) {
            return Err("Installer qualification exceeded five minutes".into());
        }
        match self.phase {
            Phase::Preparing => {
                for event in self.controller.advance_events()?.events {
                    if !matches!(event.kind, Kind::Diagnostic(_)) {
                        return Err("Installer preparation failed before native navigation".into());
                    }
                }
                if self.controller.bundle.is_some() {
                    if self.native_installation.is_some() {
                        self.phase = Phase::Native;
                    } else {
                        let mut choices = self.session.navigation.flow.choices().clone();
                        choices.mode =
                            sugarsubstitute_application::setup_flow::RuntimeMode::Attached;
                        choices.comfy_root =
                            events::path_text(self.workspace.join("missing-comfy"))?;
                        choices.models = events::path_text(self.workspace.join("models"))?;
                        choices.outputs = events::path_text(self.workspace.join("outputs"))?;
                        self.controller.accept(requests::Request {
                            id: "invalid".into(),
                            command: sugarsubstitute_application::setup_flow::Operation::Runtime(
                                choices,
                            ),
                        })?;
                        self.phase = Phase::Invalid;
                    }
                }
            }
            Phase::Invalid => {
                for event in self.controller.advance_events()?.events {
                    match event.kind {
                        Kind::Diagnostic(_) | Kind::Progress(_) => {}
                        Kind::Failed(_) if event.id.as_deref() == Some("invalid") => {
                            self.rejected_invalid = true;
                            if self.existing.is_none() {
                                return Ok(HeadlessStep::Shutdown);
                            }
                            self.phase = Phase::Native;
                        }
                        _ => {
                            return Err(
                                "Invalid environment qualification received an unexpected result"
                                    .into(),
                            );
                        }
                    }
                }
            }
            Phase::Native => {
                if !self.session.started() {
                    self.session.start(&self.controller)?;
                }
                self.session.advance(&mut self.controller, self.started.elapsed())?;
                if self.session.ready() {
                    return self.navigate();
                }
            }
        }
        Ok(HeadlessStep::Active)
    }

    /// Exercise page intent without native handles; backend storage/process ownership is real.
    fn navigate(&mut self) -> Result<HeadlessStep> {
        use sugarsubstitute_application::setup_flow::{Page, PathField, RuntimeMode};
        if self.session.navigation.flow.busy() {
            return Ok(HeadlessStep::Active);
        }
        if self.session.navigation.flow.page() != Page::Progress {
            self.session.invalidate()?;
        }
        let flow = &mut self.session.navigation.flow;
        if let Some(failure) = flow.failure() {
            return Err(format!("Native installer qualification failed: {failure:?}").into());
        }
        match flow.page() {
            Page::Language | Page::Location => {}
            Page::Runtime => flow.set_mode(RuntimeMode::Attached)?,
            Page::Configuration => {
                let root = self.existing.as_ref().ok_or("Fixture environment missing")?;
                flow.set_path(PathField::Comfy, events::path_text(root.clone())?)?;
            }
            Page::Folders => {
                self.inspected = true;
                flow.set_path(
                    PathField::Models,
                    events::path_text(self.workspace.join("models"))?,
                )?;
                flow.set_path(
                    PathField::Outputs,
                    events::path_text(self.workspace.join("outputs"))?,
                )?;
            }
            Page::Maintenance => {
                if self.native_installation.is_none() || self.controller.worker.is_some() {
                    return Err("Native maintenance failed to inspect without provisioning".into());
                }
                self.inspected = true;
            }
            Page::Progress => return Ok(HeadlessStep::Active),
            Page::Complete => {
                if self.installed.is_none() {
                    self.installed = Some(
                        self.controller
                            .installed
                            .as_ref()
                            .ok_or("Installation result missing")?
                            .root()
                            .join(sugarsubstitute_configuration::layout::ENTRY),
                    );
                }
                if self.native_installation.is_some() && self.controller.worker.is_some() {
                    return Err("Native maintenance started a provisioning worker".into());
                }
                if !self.launch {
                    return Ok(HeadlessStep::Shutdown);
                }
                if flow.close_requested() {
                    self.launched = true;
                    return Ok(HeadlessStep::Shutdown);
                }
            }
        }
        if let Some(request) = flow.primary(self.started.elapsed())? {
            self.session.navigation.submit(&mut self.controller, request)?;
        }
        Ok(HeadlessStep::Active)
    }
}
impl HeadlessLifetime for Fixture {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<()> {
        self.controller.start(wake)
    }
    fn prepare_native_runtime(&mut self, _: NonZeroUsize) -> Result<StartupPreparationStep> {
        Ok(StartupPreparationStep::Ready)
    }
    fn activity(&mut self, _: NonZeroUsize) -> Result<HeadlessStep> {
        self.progress()
    }
    fn next_deadline(&self) -> Option<Instant> {
        Some(Instant::now() + Duration::from_millis(20))
    }
    fn request_shutdown(&mut self) -> Result<()> {
        self.session.close()?;
        self.controller.close()
    }
    fn poll_shutdown(&mut self) -> Result<bool> {
        if !self.session.settled() {
            return Ok(false);
        }
        self.controller.settled()
    }
    fn finish_shutdown(&mut self) -> Result<()> {
        self.session.finish()?;
        self.controller.finish()
    }
}
