// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt the actual guarded child configuration before native surface admission.
use super::*;

impl ProductionStartup {
    /// Called only after the actual child returns Ready from authentication/capture/configuration.
    /// Adopt its complete nonclone result before validating or creating render state.
    /// Application session and its allocation remain here through successful worker joins.
    /// # Errors
    /// Missing/stale preparation closes native admission without dropping acquired custody.
    pub(super) fn adopt_prepared_startup(&mut self) -> Result<(), Box<dyn Error>> {
        if self.startup.is_none() {
            let prepared = self.child.take_prepared_startup().ok_or_else(|| {
                std::io::Error::other("Actual ready child lacks prepared startup custody")
            })?;
            self.startup = Some(prepared);
        }
        let startup = self
            .startup
            .as_ref()
            .ok_or_else(|| std::io::Error::other("Actual startup custody absent"))?;
        if self.closing
            || self.policy.closing()
            || startup.session().launch() != self.policy.launch()
        {
            return Err(std::io::Error::other(
                "Prepared startup no longer matches native admission",
            )
            .into());
        }
        if self.preparation.is_none() {
            let view = StartupView::new(startup.session().configuration().presentation());
            self.view = Some(view);
            let scope = self
                .render_scope
                .take()
                .ok_or_else(|| std::io::Error::other("Original render scope absent"))?;
            let locale = startup.session().configuration().presentation().locale().as_str();
            let render =
                PreparationDriver::new(locale, self.execution.client()?, scope, self.jobs.clone())?;
            self.preparation = Some(render);
            self.preparation
                .as_mut()
                .ok_or_else(|| std::io::Error::other("Adopted renderer absent"))?
                .set_visible(self.visible)?;
        }
        Ok(())
    }
}
