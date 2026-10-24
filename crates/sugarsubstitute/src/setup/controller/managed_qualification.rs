// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify real upstream provisioning and cancellation through the production installer owner.
use super::*;
use std::{
    num::NonZeroUsize,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::CompletionWake;
use sugarsubstitute_desktop::{
    HeadlessLifetime, HeadlessStep, StartupPreparationStep, installer::InstallerLifetime,
};

/// Use the actual upstream catalogue, download, extraction and Python verification.
/// The test watchdog is independent of production provisioning, which has no total deadline.
pub(in crate::setup) fn run(
    bundle: PathBuf,
    workspace: PathBuf,
    cancel: bool,
    distribution: &'static [u8],
) -> Result<Value> {
    let mut fixture = Fixture {
        controller: Controller::new(bundle, workspace.clone(), distribution),
        workspace,
        field: 0,
        selected: serde_json::Map::new(),
        started: false,
        cancel,
        cancelled_during_download: false,
        installed: None,
        phases: Vec::new(),
        deadline: Instant::now() + Duration::from_secs(7200),
    };
    sugarsubstitute_desktop::run_headless(&mut fixture)?;
    if cancel && !fixture.cancelled_during_download || !cancel && fixture.installed.is_none() {
        return Err("Managed provisioning qualification did not reach its required outcome".into());
    }
    Ok(json!({
        "schema_version":1, "passed":true,
        "real_upstream_provisioning":true,
        "cancelled_during_download":fixture.cancelled_during_download,
        "installed_executable":fixture.installed,
        "phases":fixture.phases,
        "process_family_and_execution_settled":true,
    }))
}

struct Fixture {
    controller: Controller,
    workspace: PathBuf,
    field: usize,
    selected: serde_json::Map<String, Value>,
    started: bool,
    cancel: bool,
    cancelled_during_download: bool,
    installed: Option<PathBuf>,
    phases: Vec<String>,
    deadline: Instant,
}
const FIELDS: [&str; 3] = ["release", "comfyVersion", "variant"];
impl Fixture {
    fn options(&mut self) -> Result<()> {
        let field = FIELDS.get(self.field).ok_or("Qualification field exhausted")?;
        self.controller.request(
            &json!({
                "version":1, "id":field, "operation":"options",
                "input":{"field":field,"selections":self.selected,"locale":"en","mirrors":false},
            })
            .to_string(),
        )
    }
    fn install(&mut self, token: &str) -> Result<()> {
        self.controller.request(
            &json!({
                "version":1,"id":"install","operation":"install","input":{
                    "locale":"en","installRoot":self.workspace.join("installed"),
                    "mode":"managed","comfyRoot":self.workspace.join("managed"),
                    "python":"","models":self.workspace.join("models"),
                    "outputs":self.workspace.join("outputs"),"selectionToken":token,
                },
            })
            .to_string(),
        )
    }
    fn progress(&mut self) -> Result<HeadlessStep> {
        for message in self.controller.advance()? {
            let value: Value = serde_json::from_str(&message)?;
            match value["event"].as_str() {
                Some("diagnostic") => continue,
                Some("progress") => {
                    let data = &value["data"];
                    if let Some(phase) = data["phase"].as_str() {
                        if !self.phases.iter().any(|old| old == phase) {
                            tracing::info!(
                                operation = "managed_install_qualification",
                                phase,
                                "Provisioning phase observed"
                            );
                            self.phases.push(phase.to_owned());
                        }
                        if self.cancel
                            && phase == "download"
                            && data["percent"]
                                .as_f64()
                                .is_some_and(|value| value > 0.0 && value < 100.0)
                        {
                            self.cancelled_during_download = true;
                            return Ok(HeadlessStep::Shutdown);
                        }
                    }
                }
                Some("completed") if self.field < FIELDS.len() => {
                    if value["id"].as_str() != Some(FIELDS[self.field]) {
                        return Err("Managed catalogue response identity changed".into());
                    }
                    let choices = value["data"].as_array().ok_or("Catalogue is not an array")?;
                    let choice = preferred(choices);
                    if self.field == 1 && choice.is_none() {
                        self.field += 1;
                        self.options()?;
                        continue;
                    }
                    let choice = choice.ok_or("No upstream provisioning option is available")?;
                    self.selected.insert(FIELDS[self.field].into(), choice["value"].clone());
                    self.field += 1;
                    if self.field == FIELDS.len() {
                        self.install(
                            choice["selectionToken"].as_str().ok_or("Selection token missing")?,
                        )?;
                    } else {
                        self.options()?;
                    }
                }
                Some("completed") if value["id"] == "install" => {
                    self.installed = Some(
                        self.controller
                            .installed
                            .as_ref()
                            .ok_or("Installed result absent")?
                            .root()
                            .join("sugarsubstitute-launcher.exe"),
                    );
                    return Ok(HeadlessStep::Shutdown);
                }
                _ => {
                    return Err(format!(
                        "Managed qualification observed unexpected result: {value}"
                    )
                    .into());
                }
            }
        }
        if Instant::now() >= self.deadline {
            return Err("Managed provisioning exceeded its qualification watchdog".into());
        }
        if !self.started && self.controller.bundle.is_some() {
            self.started = true;
            self.options()?;
        }
        Ok(HeadlessStep::Active)
    }
}
fn preferred(choices: &[Value]) -> Option<&Value> {
    choices.iter().find(|choice| choice["recommended"] == true).or_else(|| choices.first())
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
    #[test]
    fn upstream_recommendation_wins_without_inventing_a_package_choice() {
        let values = [json!({"value":"first"}), json!({"value":"recommended","recommended":true})];
        assert_eq!(preferred(&values).map(|value| &value["value"]), Some(&json!("recommended")));
        assert_eq!(preferred(&values[..1]), values.first());
        assert!(preferred(&[]).is_none());
    }
}
