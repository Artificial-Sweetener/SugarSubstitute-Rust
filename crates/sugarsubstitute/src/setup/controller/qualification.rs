// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify installer preparation and optional actual application creation on an offscreen desktop.
use super::*;
use std::{
    num::NonZeroUsize,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::CompletionWake;
use sugarsubstitute_desktop::{
    HeadlessLifetime, HeadlessStep, StartupPreparationStep, installer::InstallerLifetime,
};

pub(in crate::setup) fn run(
    bundle: PathBuf,
    workspace: PathBuf,
    existing: Option<PathBuf>,
    native_installation: Option<PathBuf>,
    launch: bool,
    distribution: &'static [u8],
) -> Result<Value> {
    let mut fixture = Fixture {
        controller: Controller::new(bundle, workspace.clone(), distribution),
        workspace,
        existing,
        native_installation,
        phase: 0,
        rejected_invalid: false,
        inspected: false,
        installed: None,
        launch,
        launched: false,
        deadline: Instant::now() + Duration::from_secs(300),
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

struct Fixture {
    controller: Controller,
    workspace: PathBuf,
    existing: Option<PathBuf>,
    native_installation: Option<PathBuf>,
    phase: u8,
    rejected_invalid: bool,
    inspected: bool,
    installed: Option<PathBuf>,
    launch: bool,
    launched: bool,
    deadline: Instant,
}
impl Fixture {
    /// Exercise the same Open command as the surface; creation is not application health.
    fn prepared(&mut self) -> Result<HeadlessStep> {
        self.installed = Some(
            self.controller
                .installed
                .as_ref()
                .ok_or("Installation result missing")?
                .root()
                .join("sugarsubstitute-launcher.exe"),
        );
        if self.launch {
            self.phase = 4;
            self.request("open", "open", json!({}))?;
            Ok(HeadlessStep::Active)
        } else {
            Ok(HeadlessStep::Shutdown)
        }
    }
    fn selection(&self, root: &std::path::Path) -> Value {
        json!({
            "locale":"en","installRoot":self.workspace.join("installed"),
            "mode":"attached","comfyRoot":root,"python":"",
            "models":self.workspace.join("models"),"outputs":self.workspace.join("outputs"),
        })
    }
    fn request(&mut self, id: &str, operation: &str, input: Value) -> Result<()> {
        self.controller
            .request(&json!({"version":1,"id":id,"operation":operation,"input":input}).to_string())
    }
    fn progress(&mut self) -> Result<HeadlessStep> {
        for message in self.controller.advance()? {
            let value: Value = serde_json::from_str(&message)?;
            if value["event"] == "diagnostic" || value["event"] == "progress" {
                continue;
            }
            match (self.phase, value["id"].as_str(), value["event"].as_str()) {
                (10, Some("location"), Some("completed")) => {
                    if value["data"]["existing"].is_null() || self.controller.worker.is_some() {
                        return Err(
                            "Native maintenance failed to inspect without provisioning".into()
                        );
                    }
                    self.inspected = true;
                    self.phase = 11;
                    self.request(
                        "maintain",
                        "maintain",
                        json!({"directory":self.native_installation}),
                    )?;
                }
                (11, Some("maintain"), Some("completed")) => {
                    if self.controller.worker.is_some() {
                        return Err("Native maintenance started a provisioning worker".into());
                    }
                    return self.prepared();
                }
                (1, Some("invalid"), Some("failed")) => {
                    self.rejected_invalid = true;
                    if let Some(existing) = &self.existing {
                        let selection = self.selection(existing);
                        self.phase = 2;
                        self.request("inspect", "runtime", selection)?;
                    } else {
                        return Ok(HeadlessStep::Shutdown);
                    }
                }
                (2, Some("inspect"), Some("completed")) => {
                    self.inspected = true;
                    self.phase = 3;
                    let selection = self
                        .selection(self.existing.as_ref().ok_or("Fixture environment missing")?);
                    self.request("install", "install", selection)?;
                }
                (3, Some("install"), Some("completed")) => {
                    return self.prepared();
                }
                (4, Some("open"), Some("completed")) => {
                    self.launched = true;
                    return Ok(HeadlessStep::Shutdown);
                }
                _ => {
                    return Err(format!(
                        "Installer qualification observed unexpected result: {value}"
                    )
                    .into());
                }
            }
        }
        if self.controller.closing {
            return Ok(HeadlessStep::Shutdown);
        }
        if Instant::now() >= self.deadline {
            return Err("Installer qualification exceeded five minutes".into());
        }
        if self.phase == 0 && self.controller.bundle.is_some() {
            if self.native_installation.is_some() {
                self.phase = 10;
                self.request(
                    "location",
                    "location",
                    json!({"directory":self.native_installation}),
                )?;
                return Ok(HeadlessStep::Active);
            }
            self.phase = 1;
            self.request(
                "invalid",
                "runtime",
                self.selection(&self.workspace.join("missing-comfy")),
            )?;
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
        self.controller.close()
    }
    fn poll_shutdown(&mut self) -> Result<bool> {
        self.controller.settled()
    }
    fn finish_shutdown(&mut self) -> Result<()> {
        self.controller.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A controller reply alone proves only the creation outcome that Open actually reports.
    #[test]
    fn launch_qualification_waits_for_open_completion() -> Result<()> {
        let mut fixture = awaiting_open();
        assert!(matches!(fixture.progress()?, HeadlessStep::Active));
        assert!(!fixture.launched);
        fixture.controller.reply(Some("open"), "progress", json!({}))?;
        assert!(matches!(fixture.progress()?, HeadlessStep::Active));
        assert!(!fixture.launched);
        fixture.controller.reply(Some("open"), "completed", json!({}))?;
        assert!(matches!(fixture.progress()?, HeadlessStep::Shutdown));
        assert!(fixture.launched);
        Ok(())
    }

    #[test]
    fn launch_qualification_rejects_failed_or_unrelated_completion() -> Result<()> {
        for (id, event) in [("open", "failed"), ("maintain", "completed")] {
            let mut fixture = awaiting_open();
            fixture.controller.reply(Some(id), event, json!({}))?;
            assert!(fixture.progress().is_err());
            assert!(!fixture.launched);
        }
        Ok(())
    }

    /// No native resources are opened; production controller output drives the observer.
    fn awaiting_open() -> Fixture {
        Fixture {
            controller: Controller::new(PathBuf::new(), PathBuf::new(), &[]),
            workspace: PathBuf::new(),
            existing: None,
            native_installation: None,
            phase: 4,
            rejected_invalid: false,
            inspected: false,
            installed: None,
            launch: true,
            launched: false,
            deadline: Instant::now() + Duration::from_secs(300),
        }
    }
}
