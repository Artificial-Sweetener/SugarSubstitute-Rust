// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose native installer navigation with the original provisioning and recovery controller.
use super::*;
use crate::setup::presentation::{Action, Interaction, View};
use std::time::Duration;
use sugarsubstitute_contracts::{execution::CompletionWake, presentation::FrameShape};
use sugarsubstitute_desktop::installer::native::{self, Lifetime};
use sugarsubstitute_ui::{form::Control, text::PaintSpec};

/// One application lifetime owns both native presentation and the existing install/update backend.
pub(in super::super) struct Native {
    controller: Controller,
    session: session::Session,
    inventory: Vec<Control>,
}
impl Native {
    /// Borrow the backend's current validated language after native retirement.
    pub(in super::super) fn locale(&self) -> &str {
        self.controller.locale()
    }
    /// Construct inert application state before native window and execution admission.
    pub(in super::super) fn new(
        bundle: PathBuf,
        workspace: PathBuf,
        distribution: &'static [u8],
        install_root: String,
    ) -> Result<Self> {
        Ok(Self {
            controller: Controller::new(bundle, workspace, distribution),
            session: session::Session::new(
                install_root,
                View {
                    surface: PaintSpec {
                        width: 1180,
                        height: 760,
                        scale: 1.0,
                        theme: sugarsubstitute_contracts::configuration::Theme::Dark,
                    },
                    // The native adapter delivers the system accent before the first page.
                    accent: sugarsubstitute_ui::accent::AccentPalette::PRODUCT,
                    now: Duration::ZERO,
                    scroll: 0.0,
                    interaction: Interaction::default(),
                    text_revisions: [0; 5],
                },
            )?,
            inventory: Action::inventory("en"),
        })
    }
}
impl Lifetime for Native {
    fn inventory(&self) -> &[Control] {
        &self.inventory
    }
    fn picker(&mut self) -> Option<native::Picker> {
        self.session.picker()
    }
    fn console(&self) -> Option<sugarsubstitute_desktop::installer::InstallerConsole<'_>> {
        self.controller.console()
    }
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<()> {
        self.controller.start(wake)?;
        self.session.start(&self.controller)
    }
    fn resize(&mut self, shape: FrameShape) -> Result<()> {
        self.session.resize(shape)
    }
    fn input(&mut self, event: native::Event) -> Result<()> {
        self.session.input(&mut self.controller, event)
    }
    fn advance(&mut self, elapsed: Duration) -> Result<()> {
        self.session.advance(&mut self.controller, elapsed)?;
        if self.controller.console.is_some() {
            self.controller.console_layout(self.session.console_layout())?;
        }
        Ok(())
    }
    fn page(&self) -> Option<native::Page<'_>> {
        self.session.page()
    }
    fn presented(&mut self) -> Result<()> {
        Ok(())
    }
    fn close_requested(&self) -> bool {
        self.session.close_requested()
    }
    fn close(&mut self) -> Result<()> {
        self.session.close()?;
        self.controller.close()
    }
    fn settled(&mut self) -> Result<bool> {
        if !self.session.settled() {
            return Ok(false);
        }
        self.controller.settled()
    }
    fn finish(&mut self) -> Result<()> {
        self.session.finish()?;
        self.controller.finish()
    }
}

#[cfg(all(test, windows))]
mod tests;

/// Offscreen exercise of the same native presentation and execution lifetime.
pub(in crate::setup) mod qualification;
