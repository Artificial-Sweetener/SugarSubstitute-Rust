// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify real upstream provisioning and cancellation through the production installer owner.
use super::*;
use std::{
    num::NonZeroUsize,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::CompletionWake;
use sugarsubstitute_desktop::{HeadlessLifetime, HeadlessStep, StartupPreparationStep};

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
        selected: [None, None],
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
    selected: [Option<String>; 2],
    started: bool,
    cancel: bool,
    cancelled_during_download: bool,
    installed: Option<PathBuf>,
    phases: Vec<String>,
    deadline: Instant,
}
use sugarsubstitute_application::setup_flow::{
    Choice, Choices, Operation as Command, OptionField, RuntimeMode,
};
const FIELDS: [OptionField; 3] =
    [OptionField::Release, OptionField::ComfyVersion, OptionField::Variant];
impl Fixture {
    fn options(&mut self) -> Result<()> {
        let field = *FIELDS.get(self.field).ok_or("Qualification field exhausted")?;
        self.controller.accept(requests::Request {
            id: format!("options-{}", self.field),
            command: Command::Options {
                field,
                release: self.selected[0].clone(),
                version: self.selected[1].clone(),
                locale: "en",
                mirrors: false,
            },
        })
    }
    fn install(&mut self, token: String) -> Result<()> {
        self.controller.accept(requests::Request {
            id: "install".into(),
            command: Command::Install(Choices {
                locale: "en",
                install_root: events::path_text(self.workspace.join("installed"))?,
                mode: RuntimeMode::Managed,
                comfy_root: events::path_text(self.workspace.join("managed"))?,
                python: String::new(),
                models: events::path_text(self.workspace.join("models"))?,
                outputs: events::path_text(self.workspace.join("outputs"))?,
                selection_token: Some(token),
            }),
        })
    }
    fn progress(&mut self) -> Result<HeadlessStep> {
        for event in self.controller.advance_events()?.events {
            match event.kind {
                Kind::Diagnostic(_) => continue,
                Kind::Progress(progress) => {
                    if let Some(phase) = progress.phase {
                        if !self.phases.contains(&phase) {
                            tracing::info!(
                                operation = "managed_install_qualification",
                                phase,
                                "Provisioning phase observed"
                            );
                            self.phases.push(phase.clone());
                        }
                        if self.cancel
                            && phase == "download"
                            && progress.percent.is_some_and(|value| value > 0.0 && value < 100.0)
                        {
                            self.cancelled_during_download = true;
                            return Ok(HeadlessStep::Shutdown);
                        }
                    }
                }
                Kind::Completed(Completed::Options(choices)) if self.field < FIELDS.len() => {
                    if event.id.as_deref() != Some(&format!("options-{}", self.field)) {
                        return Err("Managed catalogue response identity changed".into());
                    }
                    let choice = preferred(&choices);
                    if self.field == 1 && choice.is_none() {
                        self.field += 1;
                        self.options()?;
                        continue;
                    }
                    let choice = choice.ok_or("No upstream provisioning option is available")?;
                    if self.field < 2 {
                        self.selected[self.field] = Some(choice.value.clone());
                    }
                    self.field += 1;
                    if self.field == FIELDS.len() {
                        self.install(
                            choice.selection_token.clone().ok_or("Selection token missing")?,
                        )?;
                    } else {
                        self.options()?;
                    }
                }
                Kind::Completed(Completed::Installed) if event.id.as_deref() == Some("install") => {
                    self.installed = Some(
                        self.controller
                            .installed
                            .as_ref()
                            .ok_or("Installed result absent")?
                            .root()
                            .join(sugarsubstitute_configuration::layout::ENTRY),
                    );
                    return Ok(HeadlessStep::Shutdown);
                }
                Kind::Failed(message) | Kind::Unavailable(message) => return Err(message.into()),
                _ => return Err("Managed qualification received an unexpected result".into()),
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
fn preferred(choices: &[Choice]) -> Option<&Choice> {
    choices.iter().find(|choice| choice.recommended).or_else(|| choices.first())
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
        let values = [false, true].map(|recommended| Choice {
            value: recommended.to_string(),
            label: String::new(),
            description: String::new(),
            recommended,
            selection_token: None,
        });
        assert_eq!(preferred(&values).map(|choice| choice.value.as_str()), Some("true"));
        assert_eq!(preferred(&values[..1]), values.first());
        assert!(preferred(&[]).is_none());
    }
}
