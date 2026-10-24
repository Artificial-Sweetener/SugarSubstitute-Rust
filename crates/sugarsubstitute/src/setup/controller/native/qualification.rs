// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the native fresh-install route and the shared console with a provisioning boundary fixture.
use super::*;
use crate::setup::presentation::Action;
use std::time::Duration;
use sugarsubstitute_application::setup_flow::{self as flow, Page};
use sugarsubstitute_contracts::{execution::CompletionWake, presentation::FrameShape};
use sugarsubstitute_desktop::installer::native::{self as window, Lifetime};
use sugarsubstitute_ui::form::Control;

pub(in crate::setup) fn run(
    bundle: PathBuf,
    workspace: PathBuf,
    distribution: &'static [u8],
) -> Result<Value> {
    let mut fixture = Fixture::new(bundle, workspace, distribution)?;
    window::run(
        window::Window {
            visible: false,
            title: "SugarSubstitute installer qualification".into(),
            close: Action::Close.id(),
        },
        &mut fixture,
    )?;
    if !fixture.passed || !fixture.native_console || fixture.lines != 60 {
        return Err("Native installer qualification did not complete every assertion".into());
    }
    Ok(json!({"schema_version":1,"passed":true,"hidden_native_window":true,
        "shared_native_console":true,"duplicate_dom_scrollbars":false,
        "native_and_execution_retired":true,"output_records":fixture.lines}))
}
type Start = fn(&mut Native, Arc<dyn CompletionWake>) -> Result<()>;
struct Fixture {
    start: Start,
    owner: Native,
    workspace: PathBuf,
    shown: Option<Page>,
    install: Option<flow::RequestId>,
    lines: usize,
    passed: bool,
    native_console: bool,
}
impl Lifetime for Fixture {
    fn inventory(&self) -> &[Control] {
        self.owner.inventory()
    }
    fn picker(&mut self) -> Option<window::Picker> {
        self.owner.picker()
    }
    fn console(&self) -> Option<sugarsubstitute_desktop::installer::InstallerConsole<'_>> {
        self.owner.console()
    }
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<()> {
        (self.start)(&mut self.owner, wake)
    }
    fn resize(&mut self, shape: FrameShape) -> Result<()> {
        self.owner.resize(shape)
    }
    fn input(&mut self, event: window::Event) -> Result<()> {
        self.owner.input(event)
    }
    fn advance(&mut self, elapsed: Duration) -> Result<()> {
        if !self.owner.controller.closing {
            if elapsed >= Duration::from_secs(90) {
                return Err("Native installer qualification exceeded its test watchdog".into());
            }
            if let Some(id) = self.install {
                if self.lines < 60 {
                    self.lines += 1;
                    let text = format!(
                        "Provisioning record {}: 日本語, Español, 한국어, 中文",
                        self.lines
                    );
                    self.owner
                        .controller
                        .console
                        .as_mut()
                        .ok_or("Console owner absent")?
                        .append(&text)?;
                    self.owner.session.navigation.flow.progress(
                        id,
                        text,
                        Some(self.lines as f64),
                    )?;
                    self.owner.session.invalidate()?;
                } else if self.native_console {
                    self.owner.session.navigation.flow.receive(
                        id,
                        flow::Outcome::Completed(flow::Completed::Installed),
                        elapsed,
                    )?;
                    self.install = None;
                    self.owner.session.invalidate()?;
                }
            }
        }
        self.owner.advance(elapsed)?;
        if self.shown == Some(Page::Progress) {
            self.native_console |= self.owner.console().is_some_and(|console| console.view.visible);
        }
        Ok(())
    }
    fn page(&self) -> Option<window::Page<'_>> {
        self.owner.page()
    }
    fn presented(&mut self) -> Result<()> {
        let page = self.owner.session.navigation.flow.page();
        if page == Page::Progress {
            self.native_console |= self.owner.console().is_some_and(|console| console.view.visible);
        }
        if self.shown == Some(page) {
            return Ok(());
        }
        self.shown = Some(page);
        self.navigate(page)?;
        self.owner.session.invalidate()
    }
    fn close_requested(&self) -> bool {
        self.passed || self.owner.close_requested()
    }
    fn close(&mut self) -> Result<()> {
        self.owner.close()
    }
    fn settled(&mut self) -> Result<bool> {
        self.owner.settled()
    }
    fn finish(&mut self) -> Result<()> {
        self.owner.finish()
    }
}
impl Fixture {
    fn new(bundle: PathBuf, workspace: PathBuf, distribution: &'static [u8]) -> Result<Self> {
        Ok(Self {
            start: <Native as Lifetime>::start,
            owner: Native::new(
                bundle,
                workspace.clone(),
                distribution,
                events::path_text(workspace.join("installed"))?,
            )?,
            workspace,
            shown: None,
            install: None,
            lines: 0,
            passed: false,
            native_console: false,
        })
    }

    /// Supply only external provisioning results; navigation, layout, native controls and execution are real.
    fn navigate(&mut self, page: Page) -> Result<()> {
        let flow = &mut self.owner.session.navigation.flow;
        match page {
            Page::Language => {
                complete_primary(flow, flow::Completed::Locale)?;
            }
            Page::Location => {
                complete_primary(
                    flow,
                    flow::Completed::Location(flow::Location {
                        comfy_root: events::path_text(self.workspace.join("ComfyUI"))?,
                        models: events::path_text(self.workspace.join("models"))?,
                        outputs: events::path_text(self.workspace.join("outputs"))?,
                        existing: None,
                    }),
                )?;
            }
            Page::Runtime => {
                let mut next = flow.primary(Duration::ZERO)?;
                for label in ["Stable", "Current ComfyUI", "NVIDIA"] {
                    let request = next.take().ok_or("Native options request absent")?;
                    next = flow.receive(
                        request.id,
                        flow::Outcome::Completed(flow::Completed::Options(vec![flow::Choice {
                            value: label.into(),
                            label: label.into(),
                            description: "Provisioning boundary fixture".into(),
                            recommended: true,
                            selection_token: Some("fixture".into()),
                        }])),
                        Duration::ZERO,
                    )?;
                }
                if next.is_some() {
                    return Err("Native option chain did not settle".into());
                }
            }
            Page::Configuration => {
                complete_primary(flow, flow::Completed::Validated)?;
            }
            Page::Folders => {
                self.install =
                    complete_primary(flow, flow::Completed::Validated)?.map(|request| request.id);
            }
            Page::Progress => flow.set_details(true),
            Page::Complete => self.passed = true,
            Page::Maintenance => return Err("Fresh qualification entered maintenance".into()),
        }
        Ok(())
    }
}
fn complete_primary(
    flow: &mut flow::SetupFlow,
    result: flow::Completed,
) -> Result<Option<flow::Request>> {
    let request = flow.primary(Duration::ZERO)?.ok_or("Native primary request absent")?;
    Ok(flow.receive(request.id, flow::Outcome::Completed(result), Duration::ZERO)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn qualify() -> Result<()> {
        let mut fixture = Fixture::new(PathBuf::new(), PathBuf::from("qualification"), &[])?;
        fixture.start = super::super::tests::start;
        window::run(
            window::Window {
                visible: true,
                title: "Native installer all-page qualification".into(),
                close: Action::Close.id(),
            },
            &mut fixture,
        )?;
        assert!(fixture.passed && fixture.native_console);
        assert_eq!(fixture.lines, 60);
        assert_eq!(fixture.shown, Some(Page::Complete));
        assert!(fixture.owner.controller.owner.is_none());
        Ok(())
    }
    #[test]
    fn native_qualification_visits_fresh_route_and_observes_full_console_before_retirement()
    -> Result<()> {
        sugarsubstitute_desktop::qualify_installer_test(
            concat!(
                module_path!(),
                "::native_qualification_visits_fresh_route_and_observes_full_console_before_retirement"
            ),
            qualify,
        )
    }
}
