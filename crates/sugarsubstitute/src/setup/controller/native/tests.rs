// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Join the real native window, scene renderer, typed navigation and execution tree offscreen.
use super::*;
use sugarsubstitute_application::setup_flow::{Page, PathField};
use sugarsubstitute_ui::form::Input;

struct Fixture {
    owner: Native,
    phase: u8,
    picker: Option<native::Picker>,
    finished: bool,
}
impl Lifetime for Fixture {
    fn inventory(&self) -> &[Control] {
        self.owner.inventory()
    }
    fn picker(&mut self) -> Option<native::Picker> {
        // Only the shell-selection boundary is supplied by this test; no modal user UI is opened.
        self.picker = self.owner.picker();
        None
    }
    fn console(&self) -> Option<sugarsubstitute_desktop::installer::InstallerConsole<'_>> {
        Lifetime::console(&self.owner)
    }
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<()> {
        start(&mut self.owner, wake)
    }

    fn resize(&mut self, shape: FrameShape) -> Result<()> {
        self.owner.resize(shape)
    }
    fn input(&mut self, event: native::Event) -> Result<()> {
        self.owner.input(event)
    }
    fn advance(&mut self, elapsed: Duration) -> Result<()> {
        if let Some(picker) = self.picker.take() {
            self.owner
                .input(native::Event::Picked(picker, Ok(Some("Selected folder 🙂".into()))))?;
        }
        self.owner.advance(elapsed)
    }
    fn page(&self) -> Option<native::Page<'_>> {
        self.owner.page()
    }
    fn presented(&mut self) -> Result<()> {
        let page = self.owner.session.navigation.flow.page();
        match (self.phase, page) {
            (0, Page::Language) => {
                self.phase = 1;
                self.owner.input(native::Event::Control(Action::Primary.id(), Input::Invoke))?;
            }
            (1, Page::Location) => {
                self.phase = 2;
                self.owner.input(native::Event::Control(
                    Action::Browse(PathField::Installation).id(),
                    Input::Invoke,
                ))?;
            }
            (2, Page::Location)
                if self.owner.session.navigation.flow.choices().install_root
                    == "Selected folder 🙂" =>
            {
                self.phase = 3
            }
            _ => {}
        }
        Ok(())
    }
    fn close_requested(&self) -> bool {
        self.phase == 3 || self.owner.close_requested()
    }
    fn close(&mut self) -> Result<()> {
        self.owner.close()
    }
    fn settled(&mut self) -> Result<bool> {
        self.owner.settled()
    }
    fn finish(&mut self) -> Result<()> {
        self.owner.finish()?;
        assert!(self.owner.controller.owner.is_none());
        self.finished = true;
        Ok(())
    }
}
fn qualify() -> Result<()> {
    let mut fixture = Fixture {
        owner: Native::new(PathBuf::new(), PathBuf::new(), &[], "Original folder".into())?,
        phase: 0,
        picker: None,
        finished: false,
    };
    native::run(
        native::Window {
            title: "Native installer composition qualification".into(),
            visible: true,
            close: Action::Close.id(),
        },
        &mut fixture,
    )?;
    assert_eq!(fixture.phase, 3);
    assert!(fixture.finished);
    Ok(())
}
#[test]
fn real_native_installer_navigates_and_applies_picker_without_browser() -> Result<()> {
    sugarsubstitute_desktop::qualify_installer_test(
        concat!(
            module_path!(),
            "::real_native_installer_navigates_and_applies_picker_without_browser"
        ),
        qualify,
    )
}

/// Replace package/provisioning effects only; retain the real execution and native presentation owners.
pub(super) fn start(native: &mut Native, wake: Arc<dyn CompletionWake>) -> Result<()> {
    let backend = &mut native.controller;
    let owner = ExecutionOwner::new(backend.clock.clone(), wake)?;
    backend.client = Some(owner.client());
    backend.owner = Some(owner);
    backend.scope = Some(backend.create_scope(None)?);
    backend.console = Some(crate::setup::console::Output::new()?);
    native.session.start(backend)
}
mod console;
