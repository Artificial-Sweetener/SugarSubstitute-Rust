// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the actual hidden native installer and shared console with a deterministic provisioning boundary.
use super::*;
use sugarsubstitute_contracts::execution::CompletionWake;
use sugarsubstitute_desktop::installer::{
    InstallerConsole, InstallerConsoleLayout, InstallerLifetime, InstallerWindow,
};

pub(in crate::setup) fn run(
    bundle: PathBuf,
    workspace: PathBuf,
    distribution: &'static [u8],
) -> Result<Value> {
    let stop = match std::env::var("SUGARSUBSTITUTE_QUALIFY_BROWSER_STOP") {
        Ok(value) => match value.as_str() {
            "before-environment" | "before-controller" | "crash-before-controller" => Some(value),
            _ => return Err("Unknown browser qualification stop stage".into()),
        },
        Err(std::env::VarError::NotPresent) => None,
        Err(cause) => return Err(cause.into()),
    };
    let mut fixture = Fixture {
        controller: Controller::new(bundle, workspace.clone(), distribution),
        install: None,
        lines: 0,
        passed: false,
        native_console: false,
        released_profiles: 0,
        unproven_profiles: 0,
        recovered_profiles: 0,
        no_effect_profiles: 0,
        unproven_live_profiles: 0,
        stop,
        stopped: false,
        source_ready_sent: false,
        deadline: std::time::Instant::now() + std::time::Duration::from_secs(90),
    };
    let html = crate::setup::surface::html("en", &workspace.join("installed"))?
        .replacen(
            "<script>",
            r#"<script>
        window.addEventListener('error', event => window.chrome.webview.postMessage(JSON.stringify({
            version:1,id:'native_error',operation:'qualification_failed',input:event.message
        })));
        window.chrome.webview.postMessage(JSON.stringify({
            version:1,id:'native_boot',operation:'qualification_boot'
        }));
        "#,
            1,
        )
        .replace(
            "</body>",
            concat!(
                "<script>",
                include_str!("../../../../../installer/tests/native-script.js"),
                "</script></body>"
            ),
        );
    let result = sugarsubstitute_desktop::installer::run(
        InstallerWindow {
            visible: false,
            title: sugarsubstitute_contracts::localization::installer::messages("en").title.into(),
            html,
        },
        &mut fixture,
    );
    if fixture.stop.is_some() {
        let cause = result.err().ok_or("Early browser qualification did not request closure")?;
        if !cause.is::<StopRequested>() {
            return Err(cause);
        }
        if !fixture.stopped {
            return Err("Browser stop stage was not reached".into());
        }
        let expected = match fixture.stop.as_deref() {
            Some("before-environment") => {
                fixture.no_effect_profiles > 0 && fixture.unproven_profiles == 0
            }
            Some("before-controller") => fixture.unproven_profiles > 0,
            _ => false,
        };
        if !expected {
            return Err("Early browser retirement evidence differed".into());
        }
        return Ok(json!({"schema_version":1,"passed":true,"stop_stage":fixture.stop,
            "native_and_execution_retired":true,"no_effect_profiles":fixture.no_effect_profiles,
            "deferred_profiles":fixture.unproven_profiles}));
    }
    result?;
    if !fixture.passed
        || !fixture.native_console
        || fixture.lines < 60
        || fixture.released_profiles == 0
        || fixture.unproven_live_profiles != 0
    {
        return Err(format!(
            "Native installer qualification incomplete: document={}, console={}, records={}, released_profiles={}, unproven_live_profiles={}",
            fixture.passed, fixture.native_console, fixture.lines,
            fixture.released_profiles, fixture.unproven_live_profiles,
        ).into());
    }
    Ok(json!({"schema_version":1,"passed":true,"hidden_native_window":true,
        "shared_native_console":true,"duplicate_dom_scrollbars":false,
        "native_and_execution_retired":true,"output_records":fixture.lines,
        "browser_collections_released":fixture.released_profiles,"recovered_profiles":fixture.recovered_profiles}))
}
struct Fixture {
    controller: Controller,
    install: Option<String>,
    lines: usize,
    passed: bool,
    native_console: bool,
    released_profiles: usize,
    unproven_profiles: usize,
    recovered_profiles: usize,
    deadline: std::time::Instant,
    no_effect_profiles: usize,
    unproven_live_profiles: usize,
    stop: Option<String>,
    stopped: bool,
    source_ready_sent: bool,
}
impl InstallerLifetime for Fixture {
    fn messages(
        &self,
    ) -> &'static sugarsubstitute_contracts::localization::installer::InstallerMessages {
        self.controller.messages()
    }
    fn prepare_browser(&mut self) {
        self.controller.prepare_browser();
    }
    fn profile_path(&self) -> Option<PathBuf> {
        self.controller.profile_path()
    }
    fn recovering_profile(&self) -> bool {
        self.controller.recovering_profile()
    }
    fn browser_retired(
        &mut self,
        outcome: sugarsubstitute_desktop::installer::BrowserRetirement,
    ) -> Result<()> {
        if self.controller.recovering_profile() {
            self.recovered_profiles += 1;
        } else if outcome == sugarsubstitute_desktop::installer::BrowserRetirement::Unproven {
            self.unproven_live_profiles += 1;
        }
        match outcome {
            sugarsubstitute_desktop::installer::BrowserRetirement::Released => {
                self.released_profiles += 1
            }
            sugarsubstitute_desktop::installer::BrowserRetirement::Unproven => {
                self.unproven_profiles += 1
            }
            sugarsubstitute_desktop::installer::BrowserRetirement::NoEffects => {
                self.no_effect_profiles += 1
            }
        }
        self.controller.browser_retired(outcome)
    }
    fn work_settled(&mut self) -> Result<bool> {
        self.controller.work_settled()
    }
    fn environment_observed(&mut self, path: PathBuf) -> Result<()> {
        self.controller.environment_observed(path)
    }
    fn environment_ready(&self) -> bool {
        self.controller.environment_ready()
    }
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<()> {
        self.controller.start(wake)
    }
    fn console_layout(&mut self, layout: InstallerConsoleLayout) -> Result<()> {
        self.controller.console_layout(layout)
    }
    fn console(&self) -> Option<InstallerConsole<'_>> {
        self.controller.console()
    }
    fn request(&mut self, message: &str) -> Result<()> {
        let value: Value = serde_json::from_str(message)?;
        let id = value["id"].as_str().ok_or("Fixture request identity missing")?;
        match value["operation"].as_str() {
            Some("qualification_boot") => {
                tracing::info!(operation="installer_qualification_boot", "Native document JavaScript started");
                Ok(())
            },
            Some("options") => self.controller.reply(Some(id), "completed", json!([{
                "value":"fixture","label":"Provisioning boundary fixture","recommended":true,"selectionToken":"fixture",
            }])),
            Some("runtime" | "validate") => self.controller.reply(Some(id), "completed", json!({})),
            Some("install") => { self.install = Some(id.to_owned()); Ok(()) },
            Some("qualification_passed") => {
                self.passed = true;
                self.native_console = self.controller.console().is_some_and(|value| value.view.visible);
                Ok(())
            },
            Some("qualification_failed") => Err(format!("Native installer DOM assertion failed: {}", value["input"]).into()),
            _ => self.controller.request(message),
        }
    }
    fn advance(&mut self) -> Result<Vec<String>> {
        if std::time::Instant::now() >= self.deadline && !self.controller.closing {
            return Err("Native installer qualification exceeded its test deadline".into());
        }
        if self.install.is_some() && self.lines < 60 && !self.controller.closing {
            self.lines += 1;
            self.controller.reply(None, "progress", json!({
                "overall":self.lines,"status":format!("Provisioning record {}: 日本語, Español, 한국어, 中文", self.lines),
            }))?;
        }
        let mut output = self.controller.advance()?;
        if self.controller.bundle.is_some() && !self.source_ready_sent && !self.controller.closing {
            self.source_ready_sent = true;
            output.push(json!({"event":"qualification_source_ready"}).to_string());
        }
        if !self.controller.closing && !self.controller.recovering_profile() {
            let reached = match self.stop.as_deref() {
                Some("before-environment") => self.controller.profile_path().is_some(),
                Some("before-controller" | "crash-before-controller") => {
                    self.controller.environment_ready()
                }
                _ => false,
            };
            if reached {
                self.stopped = true;
                if self.stop.as_deref() == Some("crash-before-controller") {
                    std::process::exit(86);
                }
                return Err(StopRequested.into());
            }
        }
        Ok(output)
    }
    fn close(&mut self) -> Result<()> {
        self.controller.close()
    }
    fn settled(&mut self) -> Result<bool> {
        self.controller.settled()
    }
    fn finish(&mut self) -> Result<()> {
        self.controller.finish()
    }
}

/// Preserve intentional qualification cancellation separately from actual native teardown failures.
#[derive(Debug)]
struct StopRequested;
impl std::fmt::Display for StopRequested {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        output.write_str("Intentional browser qualification closure")
    }
}
impl std::error::Error for StopRequested {}
