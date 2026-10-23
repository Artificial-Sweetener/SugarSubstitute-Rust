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
    let mut fixture = Fixture {
        controller: Controller::new(bundle, workspace.clone(), distribution),
        install: None,
        lines: 0,
        passed: false,
        native_console: false,
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
    sugarsubstitute_desktop::installer::run(
        InstallerWindow {
            visible: false,
            title: "SugarSubstitute installer qualification".into(),
            html,
            profile: workspace.join("webview-v1").join(std::process::id().to_string()),
        },
        &mut fixture,
    )?;
    if !fixture.passed || !fixture.native_console || fixture.lines < 60 {
        return Err("Native installer qualification did not complete every assertion".into());
    }
    Ok(json!({"schema_version":1,"passed":true,"hidden_native_window":true,
        "shared_native_console":true,"duplicate_dom_scrollbars":false,
        "native_and_execution_retired":true,"output_records":fixture.lines}))
}
struct Fixture {
    controller: Controller,
    install: Option<String>,
    lines: usize,
    passed: bool,
    native_console: bool,
    deadline: std::time::Instant,
}
impl InstallerLifetime for Fixture {
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
        self.controller.advance()
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
